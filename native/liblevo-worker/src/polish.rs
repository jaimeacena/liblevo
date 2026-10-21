//! Pulido del Markdown que entrega el lector (Xberg): piezas generales en este orden:
//!
//! 1. quitar cabeceras y pies repetidos;
//! 2. reponer las rayas que el lector perdió, con la capa de texto del PDF (`dashes`);
//! 3. quitar adornos que el lector convierte en caracteres que el PDF no tiene;
//! 4. unir palabras partidas por guiones de fin de línea;
//! 5. marcar títulos con el estilo observado en el PDF.
//!
//! Ninguna regla depende de un documento concreto; todo se mide con `liblevo-eval`.

use crate::cleanup::{join_hyphenated_words, remove_foreign_ornaments, remove_running_matter};
use crate::dashes::restore_dashes;
use crate::headings::{OutlineEntry, StyledLine, apply, propose};
use crate::xberg_reader::normalize_markers;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PolishReport {
    pub removed_margins: usize,
    pub restored_dashes: usize,
    pub removed_ornaments: usize,
    pub joined_words: usize,
    pub headings: usize,
}

/// Aplica las limpiezas. `lines` y `page_texts` pueden estar vacíos (por ejemplo, en un escaneo):
/// entonces no se proponen títulos ni se reponen rayas ni se quitan adornos, y el resto se aplica.
pub fn polish(
    markdown: &str,
    lines: &[StyledLine],
    outline: &[OutlineEntry],
    page_texts: &HashMap<u32, String>,
) -> (String, PolishReport) {
    // Cada marca de página en su propio bloque, venga el Markdown de donde venga.
    let markdown = normalize_markers(markdown);
    let (without_margins, removed_margins) = remove_running_matter(&markdown);
    let (with_dashes, restored_dashes) = restore_dashes(&without_margins, page_texts);
    let (clean, removed_ornaments) = remove_foreign_ornaments(&with_dashes, page_texts);
    let (joined, joined_words) = join_hyphenated_words(&clean);
    let headings = propose(lines, outline);
    let result = apply(&joined, &headings);
    (
        result,
        PolishReport {
            removed_margins,
            restored_dashes,
            removed_ornaments,
            joined_words,
            headings: headings.len(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polish_cleans_margins_and_hyphens_and_marks_headings() {
        let mut doc = String::new();
        for page in 1..=4 {
            doc.push_str(&format!(
                "<!-- page: {page} -->\n\nCABECERA CORRIDA\n\nPárrafo {page} con una pala-\nbra partida y texto.\n\n"
            ));
        }
        doc.push_str("Capítulo final\n\nÚltimo texto del cuerpo.");
        let body = (0..30).map(|i| StyledLine {
            page: 1,
            text: format!("línea de cuerpo número {i} con varias palabras"),
            size: 10.0,
            bold: false,
            gap_before: Some(12.0),
            centered: false,
        });
        let mut lines: Vec<StyledLine> = body.collect();
        lines.push(StyledLine {
            page: 4,
            text: "Capítulo final".into(),
            size: 16.0,
            bold: true,
            gap_before: Some(30.0),
            centered: false,
        });
        let (out, report) = polish(&doc, &lines, &[], &HashMap::new());
        assert_eq!(report.removed_margins, 4);
        assert_eq!(report.joined_words, 4);
        assert!(!out.contains("CABECERA"));
        assert!(out.contains("palabra partida"));
        assert!(out.contains("# Capítulo final"));
    }
}
