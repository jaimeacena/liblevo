//! Tablas HTML (como las escribe el modelo visual) convertidas en tablas Markdown.
//!
//! La vista de la app y el EPUB tratan el HTML incrustado como texto, así que una tabla HTML se
//! perdería como estructura. Las celdas combinadas (`colspan`, `rowspan`) se reparten: el texto
//! queda en la primera celda y las demás quedan vacías, para que cada fila tenga las mismas
//! columnas. El texto de las celdas se conserva tal cual; solo se escapan las barras verticales.

use html5gum::{Token, Tokenizer};

/// Celdas que puede ocupar una sola celda combinada (protege contra valores absurdos).
const MAX_SPAN: usize = 64;

fn span<S: html5gum::SpanBound>(tag: &html5gum::StartTag<S>, name: &[u8]) -> usize {
    tag.attributes
        .get(name)
        .and_then(|v| std::str::from_utf8(v).ok())
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, MAX_SPAN)
}

/// Filas de celdas de una tabla HTML, con las celdas combinadas repartidas.
fn rows(html: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    // Celdas pendientes de filas de abajo por `rowspan`: (fila, columna).
    let mut reserved: Vec<(usize, usize)> = Vec::new();
    let mut cell: Option<(String, usize, usize)> = None;
    for token in Tokenizer::new(html).flatten() {
        match token {
            Token::StartTag(tag) => match tag.name.as_slice() {
                b"tr" => rows.push(Vec::new()),
                b"td" | b"th" => {
                    if rows.is_empty() {
                        rows.push(Vec::new());
                    }
                    cell = Some((
                        String::new(),
                        span(&tag, b"colspan"),
                        span(&tag, b"rowspan"),
                    ));
                }
                b"br" => {
                    if let Some((text, _, _)) = cell.as_mut() {
                        text.push(' ');
                    }
                }
                _ => {}
            },
            Token::EndTag(tag) if matches!(tag.name.as_slice(), b"td" | b"th") => {
                let Some((text, colspan, rowspan)) = cell.take() else {
                    continue;
                };
                let row_index = rows.len() - 1;
                let row = rows.last_mut().expect("hay fila");
                // Las columnas ocupadas por un `rowspan` de arriba se rellenan antes.
                while reserved.contains(&(row_index, row.len())) {
                    row.push(String::new());
                }
                let column = row.len();
                row.push(text.split_whitespace().collect::<Vec<_>>().join(" "));
                for _ in 1..colspan {
                    row.push(String::new());
                }
                for below in 1..rowspan {
                    for offset in 0..colspan {
                        reserved.push((row_index + below, column + offset));
                    }
                }
            }
            Token::String(text) => {
                if let Some((cell_text, _, _)) = cell.as_mut() {
                    cell_text.push_str(&String::from_utf8_lossy(&text));
                }
            }
            _ => {}
        }
    }
    // Huecos de `rowspan` al final de una fila.
    for (index, row) in rows.iter_mut().enumerate() {
        while reserved.contains(&(index, row.len())) {
            row.push(String::new());
        }
    }
    rows.retain(|row| !row.is_empty());
    rows
}

fn markdown_table(rows: &[Vec<String>]) -> String {
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    let line = |row: &[String]| {
        let cells: Vec<String> = (0..width)
            .map(|i| row.get(i).map_or(String::new(), |c| c.replace('|', "\\|")))
            .collect();
        format!("| {} |", cells.join(" | "))
    };
    let mut out = vec![line(&rows[0]), format!("|{}", " --- |".repeat(width))];
    out.extend(rows[1..].iter().map(|r| line(r)));
    out.join("\n")
}

/// Sustituye cada `<table>…</table>` del texto por su tabla Markdown, en un bloque propio. Una tabla
/// sin celdas se deja como estaba.
pub fn tables_to_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<table") {
        let Some(end) = rest[start..]
            .find("</table>")
            .map(|e| start + e + "</table>".len())
        else {
            break;
        };
        let found = rows(&rest[start..end]);
        out.push_str(&rest[..start]);
        if found.is_empty() {
            out.push_str(&rest[start..end]);
        } else {
            out.push_str("\n\n");
            out.push_str(&markdown_table(&found));
            out.push_str("\n\n");
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_table_becomes_a_pipe_table() {
        let html = "Antes<table><tr><th>Mineral</th><th>Fórmula</th></tr>\
            <tr><td>Pirita</td><td>Fe S<sub>2</sub></td></tr></table>Después";
        assert_eq!(
            tables_to_markdown(html),
            "Antes\n\n| Mineral | Fórmula |\n| --- | --- |\n| Pirita | Fe S2 |\n\nDespués"
        );
    }

    #[test]
    fn merged_cells_keep_every_row_the_same_width() {
        let html = "<table><tr><td rowspan=\"2\">A</td><td colspan=\"2\">B</td></tr>\
            <tr><td>c</td><td>d</td></tr></table>";
        assert_eq!(
            tables_to_markdown(html).trim(),
            "| A | B |  |\n| --- | --- | --- |\n|  | c | d |"
        );
    }

    #[test]
    fn pipes_in_cells_are_escaped_and_empty_tables_left_alone() {
        assert!(tables_to_markdown("<table><tr><td>a|b</td></tr></table>").contains("a\\|b"));
        assert_eq!(tables_to_markdown("<table></table>"), "<table></table>");
        assert_eq!(
            tables_to_markdown("<table><tr>sin cierre"),
            "<table><tr>sin cierre"
        );
    }
}
