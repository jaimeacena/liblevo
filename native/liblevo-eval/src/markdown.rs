//! Lectura tolerante de la salida Markdown de cualquier candidato.

use std::collections::BTreeMap;

/// Marca de inicio de página que escriben los ejecutores de candidatos: `<!-- page: 12 -->`.
pub const PAGE_MARKER: &str = "<!-- page:";

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Structure {
    pub headings: Vec<String>,
    pub tables: usize,
    pub images: usize,
}

/// Separa el documento por marcas de página. Devuelve un mapa vacío si no hay marcas.
pub fn split_pages(markdown: &str) -> BTreeMap<u32, String> {
    let mut pages: BTreeMap<u32, String> = BTreeMap::new();
    let mut current: Option<u32> = None;
    let mut rest = markdown;
    while let Some(start) = rest.find(PAGE_MARKER) {
        if let Some(page) = current {
            pages.entry(page).or_default().push_str(&rest[..start]);
        }
        let after = &rest[start + PAGE_MARKER.len()..];
        let Some(end) = after.find("-->") else { break };
        current = after[..end].trim().parse().ok();
        rest = &after[end + 3..];
    }
    if let Some(page) = current {
        pages.entry(page).or_default().push_str(rest);
    }
    pages
}

/// Texto visible aproximado: quita marcas de página, comentarios, etiquetas HTML, imágenes,
/// destinos de enlaces y signos de formato Markdown.
pub fn visible_text(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut chars = markdown.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                // Comentarios y etiquetas HTML desaparecen; un «<» suelto se conserva.
                let mut buffer = String::from('<');
                let mut closed = false;
                for next in chars.by_ref() {
                    buffer.push(next);
                    if next == '>' {
                        closed = true;
                        break;
                    }
                    if buffer.len() > 4096 {
                        break;
                    }
                }
                let tag_like = buffer.starts_with("<!--")
                    || buffer[1..].starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '/');
                if !(closed && tag_like) {
                    out.push_str(&buffer);
                } else {
                    out.push(' ');
                }
            }
            '!' if chars.peek() == Some(&'[') => {
                // Imagen: se descarta el texto alternativo y la ruta.
                for next in chars.by_ref() {
                    if next == ')' {
                        break;
                    }
                }
                out.push(' ');
            }
            ']' if chars.peek() == Some(&'(') => {
                for next in chars.by_ref() {
                    if next == ')' {
                        break;
                    }
                }
                out.push(' ');
            }
            '#' | '*' | '_' | '`' | '|' | '[' | ']' | '>' | '~' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// Títulos, tablas e imágenes. Reconoce tablas Markdown (línea separadora) y HTML.
pub fn structure(markdown: &str) -> Structure {
    let mut result = Structure::default();
    let mut in_fence = false;
    let mut previous_was_table_row = false;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            result
                .headings
                .push(visible_text(&trimmed[hashes..]).trim().to_string());
        }
        let is_separator = trimmed.contains('-')
            && trimmed.contains('|')
            && trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '));
        if is_separator && previous_was_table_row {
            result.tables += 1;
        }
        previous_was_table_row = trimmed.starts_with('|') || (trimmed.contains(" | "));
    }
    let lower = markdown.to_lowercase();
    result.tables += lower.matches("<table").count();
    result.images = markdown.matches("![").count() + lower.matches("<img").count();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_split_by_markers() {
        let doc = "intro\n<!-- page: 1 -->uno\n<!-- page: 2 -->dos\n<!-- page: 2 -->más";
        let pages = split_pages(doc);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[&1].trim(), "uno");
        assert_eq!(pages[&2], "dos\nmás");
        assert!(split_pages("sin marcas").is_empty());
    }

    #[test]
    fn visible_text_removes_markup_but_keeps_words() {
        let text = visible_text(
            "## Título\n**negrita** y [enlace](http://x) ![img](a.png) <b>x</b> 3 < 4",
        );
        let words = crate::text::words(&text);
        assert_eq!(
            words,
            vec!["título", "negrita", "y", "enlace", "x", "3", "4"]
        );
    }

    #[test]
    fn structure_counts_headings_tables_and_images() {
        let doc = "# Uno\n\ntexto\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n<table><tr><td>x</td></tr></table>\n\n![f](f.png)\n```\n# no es título\n```\n####### tampoco\n";
        let s = structure(doc);
        assert_eq!(s.headings, vec!["Uno"]);
        assert_eq!(s.tables, 2);
        assert_eq!(s.images, 1);
    }
}
