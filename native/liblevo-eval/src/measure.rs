//! Métricas por documento para la salida de un candidato.

use crate::manifest::{Document, Kind};
use crate::markdown::{split_pages, structure, visible_text};
use crate::reference::{PdfReference, repeated_margins, without_margins};
use crate::text::{bag_matches, char_error_rate, lcs_len, margin_key, numbers, words};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Datos de ejecución que escribe el ejecutor del candidato junto a `document.md`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Run {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub seconds: Option<f64>,
    #[serde(default)]
    pub peak_mib: Option<f64>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DocResult {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub pages_measured: u32,
    pub seconds: Option<f64>,
    pub peak_mib: Option<f64>,
    /// Palabras de la referencia presentes en la salida (0–1).
    pub words_recall: Option<f64>,
    /// Palabras de la salida presentes en la referencia (0–1).
    pub words_precision: Option<f64>,
    /// Lo mismo, solo con palabras que contienen cifras.
    pub numbers_recall: Option<f64>,
    /// Orden conservado por página: subsecuencia común / palabras de la referencia (0–1).
    pub order: Option<f64>,
    /// Tasa de error de caracteres en las páginas de referencia de escaneos (0 = perfecto).
    pub cer: Option<f64>,
    /// Líneas de la salida que repiten una cabecera o pie del original.
    pub margin_leaks: Option<usize>,
    /// Apariciones de cabeceras y pies repetidos en el original.
    pub margin_source: Option<usize>,
    /// Marcadores del PDF que aparecen como título en la salida (0–1).
    pub outline_recall: Option<f64>,
    pub outline_entries: usize,
    pub headings: usize,
    pub tables: usize,
    pub images: usize,
    pub notes: Vec<String>,
}

fn ratio(a: usize, b: usize) -> Option<f64> {
    (b > 0).then(|| a as f64 / b as f64)
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// `outer` contiene las palabras de `inner` seguidas y completas («capítulo ii» no contiene
/// «capítulo i»).
fn contains_words(outer: &str, inner: &str) -> bool {
    format!(" {outer} ").contains(&format!(" {inner} "))
}

fn normalized_title(text: &str) -> String {
    words(text).join(" ")
}

/// Líneas de la salida que repiten una cabecera o pie del original. Una clave sin letras (un
/// número de página) solo cuenta junto a un salto de página, porque un número suelto también
/// puede ser una celda o una nota.
fn margin_leaks(markdown: &str, margins: &std::collections::HashSet<String>) -> usize {
    let lines: Vec<&str> = markdown
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let is_marker = |i: usize| lines[i].starts_with(crate::markdown::PAGE_MARKER);
    (0..lines.len())
        .filter(|&i| !is_marker(i))
        .filter(|&i| {
            let key = margin_key(visible_text(lines[i]).trim());
            if key.is_empty() || !margins.contains(&key) {
                return false;
            }
            key.chars().any(char::is_alphabetic)
                || (i.saturating_sub(2)..=(i + 2).min(lines.len() - 1)).any(is_marker)
        })
        .count()
}

pub fn measure_document(
    document: &Document,
    reference: Option<&PdfReference>,
    references_dir: &Path,
    candidate_dir: &Path,
) -> DocResult {
    let mut result = DocResult {
        id: document.id.clone(),
        kind: match document.kind {
            Kind::Digital => "digital".into(),
            Kind::Scanned => "escaneado".into(),
        },
        ..DocResult::default()
    };
    let folder = candidate_dir.join(&document.id);
    if let Ok(raw) = fs::read_to_string(folder.join("run.json"))
        && let Ok(run) = serde_json::from_str::<Run>(&raw)
    {
        result.seconds = run.seconds;
        result.peak_mib = run.peak_mib;
        if let Some(note) = run.note {
            result.notes.push(note);
        }
        if run.status.as_deref() == Some("failed") {
            result.status = "fallido".into();
            return result;
        }
    }
    let Ok(markdown) = fs::read_to_string(folder.join("document.md")) else {
        result.status = "sin salida".into();
        return result;
    };
    result.status = "ok".into();
    let candidate_pages = split_pages(&markdown);
    if candidate_pages.is_empty() {
        result
            .notes
            .push("sin marcas de página: orden y escaneos no medibles".into());
    }
    let shape = structure(&markdown);
    result.headings = shape.headings.len();
    result.tables = shape.tables;
    result.images = shape.images;

    let candidate_words = words(&visible_text(&markdown));

    if let Some(reference) = reference {
        result.pages_measured = reference.pages.len() as u32;
        let (margins, occurrences) = repeated_margins(&reference.pages);
        if occurrences > 0 {
            let leaks = margin_leaks(&markdown, &margins);
            result.margin_leaks = Some(leaks);
            result.margin_source = Some(occurrences);
        }
        // Títulos esperados: lista revisada en las referencias o, si no existe, marcadores del PDF.
        let expected_titles: Vec<String> =
            match fs::read_to_string(references_dir.join(&document.id).join("headings.txt")) {
                Ok(list) => list
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(String::from)
                    .collect(),
                Err(_) => reference.outline.clone(),
            };
        result.outline_entries = expected_titles.len();
        if !expected_titles.is_empty() {
            let headings: Vec<String> =
                shape.headings.iter().map(|h| normalized_title(h)).collect();
            let found = expected_titles
                .iter()
                .map(|t| normalized_title(t))
                .filter(|title| title.chars().count() >= 3)
                .filter(|title| {
                    headings.iter().any(|h| {
                        h.chars().count() >= 3
                            && (contains_words(h, title) || contains_words(title, h))
                    })
                })
                .count();
            result.outline_recall = ratio(found, expected_titles.len());
        }
        if document.kind == Kind::Digital {
            let clean: BTreeMap<u32, String> = reference
                .pages
                .iter()
                .map(|(n, t)| (*n, without_margins(t, &margins)))
                .collect();
            let reference_words: Vec<String> = clean.values().flat_map(|t| words(t)).collect();
            let matched = bag_matches(&reference_words, &candidate_words);
            result.words_recall = ratio(matched, reference_words.len());
            result.words_precision = ratio(matched, candidate_words.len());
            let reference_numbers = numbers(&reference_words);
            result.numbers_recall = ratio(
                bag_matches(&reference_numbers, &numbers(&candidate_words)),
                reference_numbers.len(),
            );
            // `LIBLEVO_EVAL_PAGINAS` muestra el orden y las palabras recuperadas de cada página
            // (solo cifras), para separar el desorden de las palabras que faltan.
            let per_page = std::env::var_os("LIBLEVO_EVAL_PAGINAS").is_some();
            let orders: Vec<f64> = clean
                .iter()
                .filter_map(|(n, text)| {
                    let page_reference = words(text);
                    let page_candidate = words(&visible_text(candidate_pages.get(n)?));
                    (page_reference.len() >= 20).then(|| {
                        let order = lcs_len(&page_reference, &page_candidate) as f64
                            / page_reference.len() as f64;
                        if per_page {
                            let found = bag_matches(&page_reference, &page_candidate) as f64
                                / page_reference.len() as f64;
                            eprintln!(
                                "{} p{n}: palabras {:.1} % · orden {:.1} % · {} palabras",
                                document.id,
                                100.0 * found,
                                100.0 * order,
                                page_reference.len()
                            );
                        }
                        order
                    })
                })
                .collect();
            result.order = mean(&orders);
        }
    }

    if document.kind == Kind::Scanned {
        let mut reference_words = Vec::new();
        let mut page_words = Vec::new();
        let (mut orders, mut cers) = (Vec::new(), Vec::new());
        for page in &document.reference_pages {
            let path = references_dir
                .join(&document.id)
                .join(format!("page-{page:04}.md"));
            let Ok(expected) = fs::read_to_string(&path) else {
                result
                    .notes
                    .push(format!("falta la referencia de la página {page}"));
                continue;
            };
            let Some(produced) = candidate_pages.get(page) else {
                result
                    .notes
                    .push(format!("la salida no marca la página {page}"));
                continue;
            };
            let expected_text = visible_text(&expected);
            let produced_text = visible_text(produced);
            let expected_words = words(&expected_text);
            let produced_words = words(&produced_text);
            if !expected_words.is_empty() {
                orders.push(
                    lcs_len(&expected_words, &produced_words) as f64 / expected_words.len() as f64,
                );
            }
            cers.push(char_error_rate(&expected_text, &produced_text));
            reference_words.extend(expected_words);
            page_words.extend(produced_words);
        }
        if !reference_words.is_empty() {
            let matched = bag_matches(&reference_words, &page_words);
            result.words_recall = ratio(matched, reference_words.len());
            result.words_precision = ratio(matched, page_words.len());
            let reference_numbers = numbers(&reference_words);
            result.numbers_recall = ratio(
                bag_matches(&reference_numbers, &numbers(&page_words)),
                reference_numbers.len(),
            );
            result.order = mean(&orders);
            result.cer = mean(&cers);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanned(id: &str, pages: Vec<u32>) -> Document {
        Document {
            id: id.into(),
            title: String::new(),
            url: String::new(),
            sha256: "0".repeat(64),
            bytes: 0,
            license: String::new(),
            kind: Kind::Scanned,
            language: "es".into(),
            tests: String::new(),
            pages: None,
            reference_pages: pages,
            private: false,
        }
    }

    #[test]
    fn titles_match_whole_words() {
        assert!(contains_words("capítulo ii disposiciones", "capítulo ii"));
        assert!(!contains_words("capítulo ii", "capítulo i"));
    }

    #[test]
    fn bare_page_numbers_only_leak_next_to_page_breaks() {
        let margins: std::collections::HashSet<String> = ["#".to_string(), "revista".to_string()]
            .into_iter()
            .collect();
        let doc = "<!-- page: 1 -->
12
texto
más texto
otra
7
revista
fin
<!-- page: 2 -->";
        // «12» está junto a la marca, «7» no; «revista» siempre cuenta.
        assert_eq!(margin_leaks(doc, &margins), 2);
    }

    #[test]
    fn scanned_pages_are_compared_with_their_references() {
        let root = std::env::temp_dir().join(format!("liblevo-eval-m-{}", std::process::id()));
        let refs = root.join("refs");
        let cand = root.join("cand");
        fs::create_dir_all(refs.join("s")).unwrap();
        fs::create_dir_all(cand.join("s")).unwrap();
        fs::write(refs.join("s/page-0002.md"), "El año 1900 fue largo.").unwrap();
        fs::write(
            cand.join("s/document.md"),
            "<!-- page: 1 -->Portada\n<!-- page: 2 -->El ano 1900 fue largo.\n",
        )
        .unwrap();
        fs::write(cand.join("s/run.json"), r#"{"status":"ok","seconds":2.5}"#).unwrap();
        let result = measure_document(&scanned("s", vec![2]), None, &refs, &cand);
        assert_eq!(result.status, "ok");
        assert_eq!(result.seconds, Some(2.5));
        assert!((result.words_recall.unwrap() - 0.8).abs() < 1e-9);
        assert_eq!(result.numbers_recall, Some(1.0));
        assert!(result.cer.unwrap() > 0.0 && result.cer.unwrap() < 0.1);
        let missing = measure_document(&scanned("otro", vec![2]), None, &refs, &cand);
        assert_eq!(missing.status, "sin salida");
        fs::remove_dir_all(&root).unwrap();
    }
}
