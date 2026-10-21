//! Normalización y comparaciones de texto independientes del documento.

use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

/// Palabras en minúsculas y forma NFC; cualquier carácter no alfanumérico separa.
pub fn words(text: &str) -> Vec<String> {
    let normalized: String = text.nfc().collect();
    normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Palabras que contienen al menos una cifra.
pub fn numbers(words: &[String]) -> Vec<String> {
    words
        .iter()
        .filter(|w| w.chars().any(|c| c.is_ascii_digit()))
        .cloned()
        .collect()
}

/// Coincidencias de bolsa de palabras: cuántas palabras de la referencia aparecen en el candidato,
/// respetando repeticiones.
pub fn bag_matches(reference: &[String], candidate: &[String]) -> usize {
    let mut available: HashMap<&str, usize> = HashMap::new();
    for word in candidate {
        *available.entry(word.as_str()).or_default() += 1;
    }
    let mut matched = 0;
    for word in reference {
        if let Some(count) = available.get_mut(word.as_str())
            && *count > 0
        {
            *count -= 1;
            matched += 1;
        }
    }
    matched
}

/// Longitud de la subsecuencia común más larga de palabras: mide si el orden se conserva.
pub fn lcs_len(a: &[String], b: &[String]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let mut previous = vec![0usize; b.len() + 1];
    let mut current = vec![0usize; b.len() + 1];
    for x in a {
        for (j, y) in b.iter().enumerate() {
            current[j + 1] = if x == y {
                previous[j] + 1
            } else {
                previous[j + 1].max(current[j])
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// Tasa de error de caracteres (distancia de edición / longitud de la referencia), con espacios
/// colapsados y forma NFC. No distingue mayúsculas para no castigar dos veces el mismo error de
/// las métricas de palabras.
pub fn char_error_rate(reference: &str, candidate: &str) -> f64 {
    let collapse = |s: &str| -> Vec<char> {
        let normalized: String = s.nfc().collect();
        normalized
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
            .chars()
            .collect()
    };
    let a = collapse(reference);
    let b = collapse(candidate);
    if a.is_empty() {
        return if b.is_empty() { 0.0 } else { 1.0 };
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, x) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(x != y);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()] as f64 / a.len() as f64
}

/// Clave de una línea de margen: minúsculas, cifras sustituidas y espacios colapsados, para que
/// «Página 12» y «Página 13» cuenten como la misma cabecera.
pub fn margin_key(line: &str) -> String {
    let normalized: String = line.nfc().collect();
    let replaced: String = normalized
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect();
    let mut key = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    while key.contains("##") {
        key = key.replace("##", "#");
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<String> {
        words(s)
    }

    #[test]
    fn words_are_case_and_punctuation_insensitive_and_nfc() {
        assert_eq!(w("Hola, MUNDO! año"), vec!["hola", "mundo", "año"]);
        // «año» con la tilde descompuesta se normaliza igual.
        assert_eq!(w("an\u{0303}o"), vec!["año"]);
    }

    #[test]
    fn bag_matches_respects_repetitions() {
        assert_eq!(bag_matches(&w("a a b"), &w("a b c")), 2);
        assert_eq!(bag_matches(&w("a b"), &w("")), 0);
    }

    #[test]
    fn lcs_detects_reordering() {
        let reference = w("uno dos tres cuatro");
        assert_eq!(lcs_len(&reference, &w("uno dos tres cuatro")), 4);
        assert_eq!(lcs_len(&reference, &w("tres cuatro uno dos")), 2);
    }

    #[test]
    fn char_error_rate_counts_edits() {
        assert_eq!(char_error_rate("casa", "casa"), 0.0);
        assert!((char_error_rate("casa", "cosa") - 0.25).abs() < 1e-9);
        assert_eq!(char_error_rate("", "x"), 1.0);
        assert_eq!(char_error_rate("a  b\nc", "a b c"), 0.0);
    }

    #[test]
    fn margin_keys_ignore_page_numbers() {
        assert_eq!(margin_key("Página 12"), margin_key("PÁGINA  134"));
        assert_ne!(margin_key("Capítulo uno"), margin_key("Capítulo dos"));
        assert_eq!(numbers(&w("año 1905 y 3a")), vec!["1905", "3a"]);
    }
}
