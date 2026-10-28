//! Bounded, script-free source retrieval for user-requested URL imports.
use anyhow::{Context, bail, ensure};
use reqwest::{Client, Url, header, redirect};
use scraper::{ElementRef, Html, Node, Selector};
use std::time::Duration;

const MAX_DOWNLOAD: usize = 2 * 1024 * 1024;
const MAX_TEXT: usize = 120_000;

#[derive(Debug)]
pub struct SourceDocument {
    /// Keep the user's original reference even when the request follows redirects.
    pub url: String,
    pub title: String,
    pub text: String,
}

pub async fn fetch(url: &str) -> anyhow::Result<SourceDocument> {
    fetch_with_timeout(url, Duration::from_secs(30)).await
}

async fn fetch_with_timeout(url: &str, timeout: Duration) -> anyhow::Result<SourceDocument> {
    let parsed = Url::parse(url).context("source URL is invalid")?;
    validate_url(&parsed)?;
    let redirects = redirect::Policy::custom(|attempt| {
        if let Err(error) = validate_url(attempt.url()) {
            attempt.error(error.to_string())
        } else {
            redirect::Policy::limited(5).redirect(attempt)
        }
    });
    let client = Client::builder()
        .timeout(timeout)
        .redirect(redirects)
        .user_agent("Keepsake/0.1 (personal study source import)")
        .build()
        .context("build source HTTP client")?;
    let mut response = client
        .get(parsed)
        .header(
            header::ACCEPT,
            "text/html, application/xhtml+xml, text/plain, text/markdown, text/x-markdown",
        )
        .send()
        .await
        .context("fetch source URL (30 second deadline; at most 5 redirects)")?;
    ensure!(
        response.status().is_success(),
        "source returned HTTP {}",
        response.status()
    );
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .context("source has no Content-Type; expected HTML, plain text or Markdown")?
        .to_str()
        .context("source Content-Type is invalid")?
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let is_html = matches!(content_type.as_str(), "text/html" | "application/xhtml+xml");
    ensure!(
        is_html
            || matches!(
                content_type.as_str(),
                "text/plain" | "text/markdown" | "text/x-markdown"
            ),
        "unsupported source Content-Type {content_type}; expected HTML, plain text or Markdown"
    );
    if let Some(length) = response.content_length() {
        ensure!(
            length <= MAX_DOWNLOAD as u64,
            "source download exceeds 2 MiB"
        );
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.context("read source body")? {
        ensure!(
            body.len().saturating_add(chunk.len()) <= MAX_DOWNLOAD,
            "source download exceeds 2 MiB"
        );
        body.extend_from_slice(&chunk);
    }
    let body = std::str::from_utf8(&body)
        .context("source must be UTF-8 text")?
        .trim_start_matches('\u{feff}');
    let (title, text) = if is_html {
        extract_html(body)?
    } else {
        let text = body.trim().to_owned();
        let title = text
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim()
            .chars()
            .take(300)
            .collect();
        (title, text)
    };
    ensure!(
        !text.trim().is_empty(),
        "source contains no readable text; it may require JavaScript, a login, or a different URL"
    );
    ensure!(
        text.len() <= MAX_TEXT,
        "readable source text exceeds 120000 bytes; choose a shorter page or section"
    );
    Ok(SourceDocument {
        url: url.to_owned(),
        title,
        text,
    })
}

fn validate_url(url: &Url) -> anyhow::Result<()> {
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "source URL must use http or https"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "source URL must not contain credentials"
    );
    ensure!(url.host_str().is_some(), "source URL must include a host");
    Ok(())
}

fn extract_html(body: &str) -> anyhow::Result<(String, String)> {
    let document = Html::parse_document(body);
    let title_selector = Selector::parse("title").expect("static selector");
    let title = document
        .select(&title_selector)
        .next()
        .map(|title| title.text().collect::<String>())
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(300)
        .collect();
    // Main content avoids navigation and footer material on ordinary article pages.
    let main = Selector::parse("main").expect("static selector");
    let article = Selector::parse("article").expect("static selector");
    let body_selector = Selector::parse("body").expect("static selector");
    let root = document
        .select(&main)
        .next()
        .or_else(|| document.select(&article).next())
        .or_else(|| document.select(&body_selector).next())
        .unwrap_or_else(|| document.root_element());
    let mut raw = String::new();
    collect_text(root, &mut raw, 0)?;
    let text = raw
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Ok((title, text))
}

fn collect_text(element: ElementRef<'_>, output: &mut String, depth: usize) -> anyhow::Result<()> {
    ensure!(
        depth < 256,
        "source HTML is nested too deeply to extract safely"
    );
    let name = element.value().name();
    if matches!(
        name,
        "script" | "style" | "noscript" | "template" | "svg" | "head"
    ) || element.attr("hidden").is_some()
        || element.attr("aria-hidden") == Some("true")
    {
        return Ok(());
    }
    let block = matches!(
        name,
        "p" | "div"
            | "section"
            | "article"
            | "main"
            | "header"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "li"
            | "dt"
            | "dd"
            | "tr"
            | "blockquote"
            | "pre"
            | "br"
            | "hr"
    );
    let cell = matches!(name, "td" | "th");
    if block {
        output.push('\n');
    }
    if cell {
        output.push(' ');
    }
    for child in element.children() {
        match child.value() {
            Node::Text(text) => output.push_str(text),
            Node::Element(_) => {
                if let Some(child) = ElementRef::wrap(child) {
                    collect_text(child, output, depth + 1)?;
                }
            }
            Node::Document
            | Node::Fragment
            | Node::Doctype(_)
            | Node::Comment(_)
            | Node::ProcessingInstruction(_) => {}
        }
    }
    if block {
        output.push('\n');
    }
    if cell {
        output.push(' ');
    }
    // The input is capped already; this also bounds separators added during traversal.
    if output.len() > MAX_DOWNLOAD * 2 {
        bail!("source HTML expands beyond the extraction limit");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn fixture(response: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).await;
            let _ = stream.write_all(&response).await;
        });
        format!("http://{address}/source")
    }

    async fn document(kind: &str, body: &str) -> String {
        let response = format!(
            concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: {kind}\r\n",
                "Content-Length: {length}\r\n",
                "Connection: close\r\n",
                "\r\n",
                "{body}",
            ),
            kind = kind,
            length = body.len(),
            body = body,
        );
        fixture(response.into_bytes()).await
    }

    #[tokio::test]
    async fn html_keeps_article_text_and_omits_non_content() {
        let html = concat!(
            "<html><head><title>Notes &amp; examples</title></head><body>",
            "<nav>Navigation</nav>",
            "<main><h1>SQLite</h1>",
            "<p>WAL means <strong>write-ahead logging</strong>.</p>",
            "<script>do bad things</script>",
            "<style>secret css</style>",
            "<noscript>fallback</noscript>",
            "<template>template text</template>",
            "<svg><text>vector text</text></svg>",
            "<p hidden>hidden text</p>",
            "<p aria-hidden=\"true\">not readable</p>",
            "<p>Changes enter the log first.</p></main>",
            "<footer>Footer</footer></body></html>",
        );
        let url = document("text/html; charset=utf-8", html).await;
        let fetched = fetch(&url).await.unwrap();
        assert_eq!(fetched.url, url);
        assert_eq!(fetched.title, "Notes & examples");
        assert_eq!(
            fetched.text,
            "SQLite\nWAL means write-ahead logging.\nChanges enter the log first."
        );
    }

    #[tokio::test]
    async fn supports_markdown_and_plain_text_with_bom() {
        for kind in ["text/plain", "text/markdown", "text/x-markdown"] {
            let url = document(kind, "\u{feff}# Study notes\n\nRecall first.\n").await;
            let fetched = fetch(&url).await.unwrap();
            assert_eq!(fetched.title, "# Study notes");
            assert_eq!(fetched.text, "# Study notes\n\nRecall first.");
        }
    }

    #[test]
    fn preserves_table_word_boundaries_and_rejects_pathological_nesting() {
        let (_, text) =
            extract_html("<table><tr><td>Capital</td><td>Paris</td></tr></table>").unwrap();
        assert_eq!(text, "Capital Paris");
        let deep = format!("{}text{}", "<div>".repeat(300), "</div>".repeat(300));
        assert!(extract_html(&deep).is_err());
    }

    #[tokio::test]
    async fn rejects_bad_status_types_credentials_and_empty_documents() {
        let url = fixture(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec()).await;
        assert!(fetch(&url).await.unwrap_err().to_string().contains("404"));
        let url = document("application/pdf", "%PDF").await;
        assert!(
            fetch(&url)
                .await
                .unwrap_err()
                .to_string()
                .contains("Content-Type")
        );
        let url = fixture(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntext".to_vec()).await;
        assert!(
            fetch(&url)
                .await
                .unwrap_err()
                .to_string()
                .contains("Content-Type")
        );
        let url = document("text/html", "<script>Only scripts</script>").await;
        assert!(
            fetch(&url)
                .await
                .unwrap_err()
                .to_string()
                .contains("no readable text")
        );
        for invalid in [
            "file:///tmp/study",
            "https://user:secret@example.com/",
            "not a URL",
        ] {
            assert!(fetch(invalid).await.is_err());
        }
    }

    #[tokio::test]
    async fn bounds_download_and_readable_text_without_truncation() {
        let url = fixture(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n",
                MAX_DOWNLOAD + 1
            )
            .into_bytes(),
        )
        .await;
        assert!(fetch(&url).await.unwrap_err().to_string().contains("2 MiB"));
        let url = document("text/plain", &"x".repeat(MAX_TEXT + 1)).await;
        assert!(
            fetch(&url)
                .await
                .unwrap_err()
                .to_string()
                .contains("120000")
        );
        let mut response =
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n".to_vec();
        response.extend(vec![b'x'; MAX_DOWNLOAD + 1]);
        let url = fixture(response).await;
        assert!(fetch(&url).await.unwrap_err().to_string().contains("2 MiB"));
    }

    #[tokio::test]
    async fn follows_redirect_preserving_original_reference_and_limits_loops() {
        let target = document("text/plain", "Source facts").await;
        let response = format!(
            concat!(
                "HTTP/1.1 302 Found\r\n",
                "Location: {target}\r\n",
                "Content-Length: 0\r\n",
                "Connection: close\r\n\r\n",
            ),
            target = target,
        );
        let original = fixture(response.into_bytes()).await;
        assert_eq!(fetch(&original).await.unwrap().url, original);

        let credentials = fixture(
            concat!(
                "HTTP/1.1 302 Found\r\n",
                "Location: http://user:secret@127.0.0.1:1/\r\n",
                "Content-Length: 0\r\n",
                "Connection: close\r\n\r\n",
            )
            .as_bytes()
            .to_vec(),
        )
        .await;
        assert!(
            format!("{:#}", fetch(&credentials).await.unwrap_err())
                .contains("must not contain credentials")
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            for _ in 0..6 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request).await;
                let response = concat!(
                    "HTTP/1.1 302 Found\r\n",
                    "Location: /loop\r\n",
                    "Content-Length: 0\r\n",
                    "Connection: close\r\n\r\n",
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        assert!(fetch(&format!("http://{address}/loop")).await.is_err());
        task.abort();
    }

    #[tokio::test]
    async fn total_deadline_covers_a_stalled_body() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).await;
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 10\r\n\r\nx",
                )
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let start = std::time::Instant::now();
        assert!(
            fetch_with_timeout(
                &format!("http://{address}/slow"),
                Duration::from_millis(100)
            )
            .await
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(2));
        task.abort();
    }
}
