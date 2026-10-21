//! Señales de duda por página. Deciden qué páginas de un escaneo se releen con el modelo visual y,
//! más adelante, qué ve la persona en el revisor.
//!
//! Señal actual: desacuerdo entre el OCR y la capa de texto invisible que traen muchos escaneos. Si
//! los dos lectores no coinciden en al menos un 80 % de las palabras, al menos uno se equivoca y la
//! página es dudosa. Medido el 19 de octubre de 2026 en las páginas de referencia de los escaneos:
//! separaba la única página en la que el modelo visual mejora claramente (una tabla, 70 %) del resto
//! (87 % o más). Una muestra tan pequeña puede descartar, no aprobar: ninguna ausencia de señal
//! certifica que una página sea correcta, y las páginas sin capa invisible no tienen señal.

use std::collections::HashMap;

/// Coincidencia mínima entre el OCR y la capa invisible para no dudar de una página.
const MIN_AGREEMENT: f64 = 0.80;
/// Palabras mínimas en cada lectura para que la comparación diga algo.
const MIN_WORDS: usize = 20;

fn word_counts(text: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        *counts.entry(word.to_lowercase()).or_insert(0) += 1;
    }
    counts
}

/// Parte de las palabras de `a` que también están en `b` (contando repeticiones).
fn covered(a: &HashMap<String, usize>, b: &HashMap<String, usize>) -> f64 {
    let total: usize = a.values().sum();
    let shared: usize = a
        .iter()
        .map(|(word, n)| (*n).min(b.get(word).copied().unwrap_or(0)))
        .sum();
    shared as f64 / total.max(1) as f64
}

/// Coincidencia entre dos lecturas de una página: la menor de las dos direcciones, o `None` si
/// alguna tiene muy pocas palabras para compararlas.
pub fn agreement(ocr: &str, layer: &str) -> Option<f64> {
    let (a, b) = (word_counts(ocr), word_counts(layer));
    if a.values().sum::<usize>() < MIN_WORDS || b.values().sum::<usize>() < MIN_WORDS {
        return None;
    }
    Some(covered(&a, &b).min(covered(&b, &a)))
}

/// Texto de cada página del Markdown con marcas `<!-- page: N -->`.
pub fn markdown_pages(markdown: &str) -> HashMap<u32, String> {
    let mut pages: HashMap<u32, String> = HashMap::new();
    let mut current: Option<u32> = None;
    for block in markdown.split("\n\n") {
        if let Some(rest) = block.trim().strip_prefix("<!-- page:") {
            current = rest.trim_end_matches("-->").trim().parse().ok();
            continue;
        }
        if let Some(page) = current {
            let text = pages.entry(page).or_default();
            text.push_str(block);
            text.push_str("\n\n");
        }
    }
    pages
}

/// Páginas dudosas, en orden: el OCR y la capa invisible (`layer_texts`, texto completo de cada
/// página según PDFium) coinciden en menos del 80 % de las palabras.
pub fn doubtful_pages(markdown: &str, layer_texts: &HashMap<u32, String>) -> Vec<u32> {
    let mut pages: Vec<u32> = markdown_pages(markdown)
        .iter()
        .filter_map(|(page, text)| {
            let layer = layer_texts.get(page)?;
            agreement(text, layer)
                .filter(|a| *a < MIN_AGREEMENT)
                .map(|_| *page)
        })
        .collect();
    pages.sort_unstable();
    pages
}

/// Tinta mínima (parte de la zona central de la página) para sospechar una figura.
const MIN_FIGURE_INK: f64 = 0.02;
/// Veces que la tinta de una página debe superar la que explican sus palabras.
const FIGURE_INK_EXCESS: f64 = 2.5;
/// Palabras mínimas de una página para calibrar la tinta por palabra del documento.
const CALIBRATION_WORDS: usize = 50;

/// Páginas de un escaneo que probablemente tienen una figura: mucha más tinta de la que explican
/// sus palabras. La tinta por palabra se calibra con el propio documento (mediana de sus páginas
/// de texto). Medido el 19 de octubre de 2026 en los escaneos del conjunto: encuentra ilustraciones,
/// sellos, portadas y fotografías; también marca alguna página de texto denso, que solo cuesta una
/// relectura. `ink[i]` es la tinta de la página `i + 1` (`pdf_images::ink_fractions`).
pub fn figure_pages(ink: &[f64], words_per_page: &HashMap<u32, usize>) -> Vec<u32> {
    let words = |page: u32| words_per_page.get(&page).copied().unwrap_or(0);
    let mut per_word: Vec<f64> = ink
        .iter()
        .enumerate()
        .filter(|(i, _)| words(*i as u32 + 1) >= CALIBRATION_WORDS)
        .map(|(i, ink)| ink / words(i as u32 + 1) as f64)
        .collect();
    per_word.sort_by(f64::total_cmp);
    let typical = per_word.get(per_word.len() / 2).copied().unwrap_or(0.0);
    ink.iter()
        .enumerate()
        .filter(|(i, ink)| {
            let expected = typical * words(*i as u32 + 1).max(20) as f64;
            **ink > MIN_FIGURE_INK && **ink > FIGURE_INK_EXCESS * expected
        })
        .map(|(i, _)| i as u32 + 1)
        .collect()
}

/// Palabras de cada página del Markdown con marcas.
pub fn words_per_page(markdown: &str) -> HashMap<u32, usize> {
    markdown_pages(markdown)
        .into_iter()
        .map(|(page, text)| {
            let count = text
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty())
                .count();
            (page, count)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn pages_with_more_ink_than_their_words_explain_are_figures() {
        // Diez páginas de texto: 0,1 % de tinta por cada 10 palabras.
        let mut ink: Vec<f64> = vec![0.03; 10];
        let mut words: HashMap<u32, usize> = (1..=10).map(|p| (p, 300)).collect();
        // Página 11: lámina con pie de 8 palabras; página 12: página en blanco limpia.
        ink.push(0.25);
        words.insert(11, 8);
        ink.push(0.005);
        words.insert(12, 0);
        // Página 13: texto con una figura pequeña (el doble de tinta no basta; el triple sí).
        ink.push(0.09);
        words.insert(13, 300);
        assert_eq!(figure_pages(&ink, &words), vec![11, 13]);
        assert_eq!(
            words_per_page("<!-- page: 1 -->\n\nUna dos tres.\n\n<!-- page: 2 -->\n\n"),
            HashMap::from([(1, 3), (2, 0)])
        );
    }

    use super::*;

    const TEXT: &str = "It was the best of times, it was the worst of times, it was the age of \
        wisdom, it was the age of foolishness, it was the epoch of belief, it was the epoch of \
        incredulity, it was the season of Light";

    #[test]
    fn matching_readings_agree() {
        let a = agreement(TEXT, &TEXT.replace(',', "")).unwrap();
        assert!(a > 0.99);
    }

    #[test]
    fn too_few_words_give_no_signal() {
        assert_eq!(agreement("Capítulo uno", TEXT), None);
        assert_eq!(agreement(TEXT, ""), None);
    }

    #[test]
    fn disagreement_marks_only_that_page() {
        let garbled = "lt wa5 tbe bcst 0f tirnes it wa5 tbe w0rst 0f tirnes it wa5 tbe agc 0f \
            wisd0m it wa5 tbe agc 0f f00lishness it wa5 tbe ep0ch 0f bclief";
        let markdown = format!("<!-- page: 1 -->\n\n{TEXT}\n\n<!-- page: 2 -->\n\n{garbled}");
        let layers = HashMap::from([(1, TEXT.to_string()), (2, TEXT.to_string())]);
        assert_eq!(doubtful_pages(&markdown, &layers), vec![2]);
        // Sin capa invisible no hay señal, ni siquiera para la página mal leída.
        let only_first = HashMap::from([(1, TEXT.to_string())]);
        assert!(doubtful_pages(&markdown, &only_first).is_empty());
    }
}
