//! Text a model reads well: pipe tables and bounded cells.

/// A pipe table. Cells are flattened to one line, so a newline or `|` in a question
/// cannot break a row.
pub(crate) fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let line = |cells: Vec<String>| format!("| {} |", cells.join(" | "));
    let mut out = vec![
        line(headers.iter().map(|header| header.to_string()).collect()),
        line(headers.iter().map(|_| "---".to_string()).collect()),
    ];
    out.extend(
        rows.iter()
            .map(|row| line(row.iter().map(|cell| cell_text(cell)).collect())),
    );
    out.join("\n")
}

fn cell_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

/// At most `limit` characters, marked when cut so it is never quoted as the whole text.
pub(crate) fn clip(value: &str, limit: usize) -> String {
    match value.char_indices().nth(limit) {
        Some((end, _)) => format!("{}…", &value[..end]),
        None => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_cannot_break_their_row() {
        let rendered = table(&["id", "question"], &[vec!["1".into(), "A | B\nC".into()]]);
        assert_eq!(
            rendered,
            "| id | question |\n| --- | --- |\n| 1 | A \\| B C |"
        );
    }

    #[test]
    fn clipping_marks_what_it_cuts_and_respects_characters() {
        assert_eq!(clip("kererū", 6), "kererū");
        assert_eq!(clip("kererū", 5), "kerer…");
        assert_eq!(clip("āēīōū", 2), "āē…");
    }
}
