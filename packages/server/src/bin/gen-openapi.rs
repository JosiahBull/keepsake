use keepsake_server::ApiDoc;
use utoipa::OpenApi;

fn main() -> anyhow::Result<()> {
    let destination = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "packages/client/openapi.json".into());
    std::fs::write(destination, ApiDoc::openapi().to_pretty_json()?)?;
    Ok(())
}
