//! Detección de PDF escaneados: decide si la lectura usa OCR en vez del texto del PDF.
//!
//! Una página cuenta como escaneada si no tiene texto visible y una imagen cubre la mayor parte de
//! ella. El texto invisible (la capa OCR que añaden algunos escáneres) no cuenta como texto: el
//! OCR propio es más fiable que esa capa en el conjunto de evaluación. Las páginas en blanco o solo
//! con dibujos no votan. El documento se lee con OCR si al menos la mitad de las páginas que votan
//! son escaneos.

use crate::headings::StyledLine;
use pdfium_render::prelude::*;
use std::collections::HashMap;

/// Letras visibles a partir de las cuales una página tiene texto propio.
const MIN_VISIBLE_LETTERS: usize = 20;
/// Parte de la página que debe cubrir una imagen para tratarla como página escaneada.
const MIN_IMAGE_COVERAGE: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanVote {
    pub text_pages: usize,
    pub scanned_pages: usize,
}

impl ScanVote {
    pub fn is_scanned(self) -> bool {
        self.scanned_pages > 0 && self.scanned_pages * 2 >= self.scanned_pages + self.text_pages
    }
}

/// Letras visibles por página (1 = primera), según las líneas con estilo ya leídas.
fn visible_letters(lines: &[StyledLine]) -> HashMap<u32, usize> {
    let mut letters = HashMap::new();
    for line in lines {
        *letters.entry(line.page).or_insert(0) +=
            line.text.chars().filter(|c| c.is_alphanumeric()).count();
    }
    letters
}

/// Parte de la página cubierta por su imagen más grande (0 si no tiene).
fn largest_image_coverage(page: &PdfPage) -> f64 {
    let area = f64::from(page.width().value) * f64::from(page.height().value);
    if area <= 0.0 {
        return 0.0;
    }
    page.objects()
        .iter()
        .filter(|object| object.object_type() == PdfPageObjectType::Image)
        .filter_map(|object| object.bounds().ok())
        .map(|bounds| f64::from(bounds.width().value) * f64::from(bounds.height().value) / area)
        .fold(0.0, f64::max)
}

/// Cuenta páginas con texto y páginas escaneadas. `lines` son las líneas visibles del documento
/// (`pdf_styles::styled_lines` con la primera página = 1).
pub fn vote(
    pdfium: &Pdfium,
    path: &std::path::Path,
    lines: &[StyledLine],
) -> Result<ScanVote, String> {
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("PDF: {e:?}"))?;
    let letters = visible_letters(lines);
    let mut vote = ScanVote::default();
    for (index, page) in document.pages().iter().enumerate() {
        let number = index as u32 + 1;
        if letters.get(&number).copied().unwrap_or(0) >= MIN_VISIBLE_LETTERS {
            vote.text_pages += 1;
        } else if largest_image_coverage(&page) >= MIN_IMAGE_COVERAGE {
            vote.scanned_pages += 1;
        }
    }
    Ok(vote)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(page: u32, text: &str) -> StyledLine {
        StyledLine {
            page,
            text: text.into(),
            size: 10.0,
            bold: false,
            gap_before: None,
            centered: false,
        }
    }

    #[test]
    fn majority_of_scanned_pages_decides() {
        let vote = |text_pages, scanned_pages| ScanVote {
            text_pages,
            scanned_pages,
        };
        assert!(vote(0, 10).is_scanned());
        assert!(vote(5, 5).is_scanned());
        assert!(!vote(6, 4).is_scanned());
        // Sin páginas escaneadas nunca es escaneo, ni siquiera un PDF vacío.
        assert!(!vote(0, 0).is_scanned());
        assert!(!vote(9, 0).is_scanned());
    }

    #[test]
    fn letters_are_counted_per_page() {
        let lines = [line(1, "Uno dos"), line(1, "tres"), line(3, "— 12 —")];
        let letters = visible_letters(&lines);
        assert_eq!(letters[&1], 10);
        assert_eq!(letters[&3], 2);
        assert!(!letters.contains_key(&2));
    }
}
