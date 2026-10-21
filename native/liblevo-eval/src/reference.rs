//! Referencias extraídas del propio PDF con PDFium: texto por página y marcadores.

use crate::text::margin_key;
use pdfium_render::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

pub struct PdfReference {
    /// Texto de la capa del PDF por página (1-based) dentro del rango medido.
    pub pages: BTreeMap<u32, String>,
    /// Títulos de los marcadores del PDF.
    pub outline: Vec<String>,
}

pub fn read(
    pdfium: &Pdfium,
    path: &Path,
    range: impl Fn(u32) -> (u32, u32),
) -> Result<PdfReference, String> {
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("No se pudo abrir {}: {e:?}", path.display()))?;
    let page_count = u32::try_from(document.pages().len()).unwrap_or(0);
    let (first, last) = range(page_count);
    let mut pages = BTreeMap::new();
    for number in first..=last {
        let page = document
            .pages()
            .get((number - 1) as PdfPageIndex)
            .map_err(|e| format!("Página {number}: {e:?}"))?;
        let text = page.text().map(|t| t.all()).unwrap_or_default();
        pages.insert(number, text);
    }
    let outline = document
        .bookmarks()
        .iter()
        .filter_map(|bookmark| bookmark.title())
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .collect();
    Ok(PdfReference { pages, outline })
}

/// Líneas de margen repetidas: entre las dos primeras y dos últimas líneas no vacías de cada
/// página, las claves que aparecen en al menos 3 páginas y en el 15 % de las medidas.
/// Devuelve las claves y cuántas veces aparecen en el original.
pub fn repeated_margins(pages: &BTreeMap<u32, String>) -> (HashSet<String>, usize) {
    let mut pages_with_key: HashMap<String, usize> = HashMap::new();
    for text in pages.values() {
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let mut keys = HashSet::new();
        let n = lines.len();
        for (i, line) in lines.iter().enumerate() {
            if i < 2 || i + 2 >= n {
                keys.insert(margin_key(line));
            }
        }
        for key in keys {
            *pages_with_key.entry(key).or_default() += 1;
        }
    }
    let threshold = 3.max((pages.len() as f64 * 0.15).ceil() as usize);
    let repeated: HashSet<String> = pages_with_key
        .iter()
        .filter(|(key, count)| **count >= threshold && !key.is_empty())
        .map(|(key, _)| key.clone())
        .collect();
    let occurrences = repeated.iter().map(|key| pages_with_key[key]).sum();
    (repeated, occurrences)
}

/// Texto de la página sin sus líneas de margen repetidas.
pub fn without_margins(text: &str, margins: &HashSet<String>) -> String {
    text.lines()
        .filter(|line| !margins.contains(&margin_key(line.trim())))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_margins_need_repetition_across_pages() {
        let words = ["alfa", "beta", "gamma", "delta", "épsilon", "zeta", "eta"];
        let mut pages = BTreeMap::new();
        for n in 1..=6u32 {
            let a = words[n as usize];
            let b = words[(n as usize + 3) % 7];
            pages.insert(
                n,
                format!("REVISTA DE PRUEBA\nEmpieza {a}\nsigue\nsigue\nacaba {b}\n{n}"),
            );
        }
        let (keys, occurrences) = repeated_margins(&pages);
        assert!(keys.contains(&margin_key("REVISTA DE PRUEBA")));
        assert!(keys.contains("#"));
        assert!(
            !keys
                .iter()
                .any(|k| k.starts_with("empieza") || k.starts_with("acaba"))
        );
        assert_eq!(occurrences, 12);
        let cleaned = without_margins(&pages[&1], &keys);
        assert!(!cleaned.contains("REVISTA"));
        assert!(cleaned.contains("Empieza beta"));
    }
}
