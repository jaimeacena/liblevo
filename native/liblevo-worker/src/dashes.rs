//! Restauración de rayas (— y –) que el lector pierde, con la capa de texto visible del propio PDF
//! como autoridad.
//!
//! El modo de maquetación de Xberg a veces quita una raya y pega las palabras («flicker—may» →
//! «flickermay») o la cambia por un espacio. Para cada raya que PDFium ve entre dos palabras de una
//! página se buscan, en la misma página del Markdown, esas dos palabras sin raya. Solo se repone
//! cuando el caso es inequívoco: faltan tantas rayas de ese par como candidatos hay. Si el mismo par
//! aparece también sin raya de forma legítima, no se toca nada.

use std::collections::HashMap;

const DASHES: [char; 2] = ['\u{2014}', '\u{2013}'];
/// Signos que pueden ir entre la raya y la palabra siguiente (comillas o paréntesis de apertura).
const OPENERS: [char; 10] = ['“', '"', '‘', '\'', '«', '¿', '¡', '(', '[', '_'];

/// Una raya entre dos palabras tal como la muestra el PDF.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DashPair {
    left: String,
    right: String,
    dash: char,
    /// Espacio entre la palabra izquierda y la raya («ago—» frente a «ago —»).
    spaced_before: bool,
    /// Espacio entre la raya y la palabra derecha cuando están en la misma línea; `None` si la
    /// raya cierra la línea.
    spaced_after: Option<bool>,
}

fn word_before(chars: &[char], end: usize) -> &[char] {
    let mut start = end;
    while start > 0 && chars[start - 1].is_alphanumeric() {
        start -= 1;
    }
    &chars[start..end]
}

/// Rayas entre palabras de un texto de página (líneas unidas con saltos de línea).
fn pairs(text: &str) -> Vec<DashPair> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    for (i, &c) in chars.iter().enumerate() {
        if !DASHES.contains(&c) {
            continue;
        }
        // Una raya doble o repetida no es una raya entre dos palabras.
        if (i > 0 && DASHES.contains(&chars[i - 1]))
            || chars.get(i + 1).is_some_and(|n| DASHES.contains(n))
        {
            continue;
        }
        let mut end = i;
        while end > 0 && chars[end - 1] == ' ' {
            end -= 1;
        }
        let left = word_before(&chars, end);
        if left.is_empty() {
            continue;
        }
        let mut j = i + 1;
        let mut gap = String::new();
        while j < chars.len() && (chars[j] == ' ' || chars[j] == '\n') {
            gap.push(chars[j]);
            j += 1;
        }
        while j < chars.len() && OPENERS.contains(&chars[j]) {
            j += 1;
        }
        let start = j;
        while j < chars.len() && chars[j].is_alphanumeric() {
            j += 1;
        }
        if start == j {
            continue;
        }
        out.push(DashPair {
            left: left.iter().collect(),
            right: chars[start..j].iter().collect(),
            dash: c,
            spaced_before: end < i,
            spaced_after: (!gap.contains('\n')).then_some(!gap.is_empty()),
        });
    }
    out
}

/// Dónde aparece el par en un trozo de Markdown.
#[derive(Debug, PartialEq, Eq)]
enum Found {
    /// Ya tiene su raya.
    Dashed,
    /// Las dos palabras pegadas; la raya va en `at` (posición en bytes).
    Glued { at: usize },
    /// Separadas por espacios o saltos (`gap` en bytes desde `at`) y quizá signos de apertura.
    Separated { at: usize, gap: usize },
}

fn is_word_char(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

fn occurrences(text: &str, pair: &DashPair) -> Vec<Found> {
    let mut out = Vec::new();
    for (start, _) in text.match_indices(pair.left.as_str()) {
        if is_word_char(text[..start].chars().next_back()) {
            continue;
        }
        let after_left = start + pair.left.len();
        let rest = &text[after_left..];
        let right_at = |s: &str| {
            s.strip_prefix(pair.right.as_str())
                .is_some_and(|tail| !is_word_char(tail.chars().next()))
        };
        if right_at(rest) {
            out.push(Found::Glued { at: after_left });
            continue;
        }
        let gap = rest.len() - rest.trim_start_matches([' ', '\n']).len();
        let spaced = &rest[gap..];
        if let Some(after_dash) = spaced.strip_prefix(DASHES) {
            let tail = after_dash
                .trim_start_matches([' ', '\n'])
                .trim_start_matches(OPENERS);
            if right_at(tail) {
                out.push(Found::Dashed);
            }
            continue;
        }
        if gap > 0 && right_at(spaced.trim_start_matches(OPENERS)) {
            out.push(Found::Separated {
                at: after_left,
                gap,
            });
        }
    }
    out
}

/// Repone las rayas perdidas de una página. Devuelve el texto y cuántas repuso.
fn restore_page(text: &str, page_pairs: &[DashPair]) -> (String, usize) {
    let mut wanted: HashMap<&DashPair, usize> = HashMap::new();
    for pair in page_pairs {
        *wanted.entry(pair).or_insert(0) += 1;
    }
    // Inserciones como (posición, bytes a sustituir, texto nuevo).
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (pair, count) in wanted {
        let found = occurrences(text, pair);
        let dashed = found.iter().filter(|f| **f == Found::Dashed).count();
        let candidates: Vec<&Found> = found.iter().filter(|f| **f != Found::Dashed).collect();
        let missing = count.saturating_sub(dashed);
        if missing == 0 || candidates.len() != missing {
            continue;
        }
        let before = if pair.spaced_before { " " } else { "" };
        for candidate in candidates {
            match *candidate {
                Found::Glued { at } => {
                    let after = if pair.spaced_after == Some(true) {
                        " "
                    } else {
                        ""
                    };
                    edits.push((at, 0, format!("{before}{}{after}", pair.dash)));
                }
                Found::Separated { at, gap } => {
                    let gap_text = &text[at..at + gap];
                    // Raya cerrada («since—you»): sobra el espacio que puso el lector, salvo un
                    // salto de párrafo, que se conserva.
                    let closed = !pair.spaced_before && pair.spaced_after != Some(true);
                    let replaced = if closed && !gap_text.contains('\n') {
                        gap
                    } else {
                        0
                    };
                    edits.push((at, replaced, format!("{before}{}", pair.dash)));
                }
                Found::Dashed => {}
            }
        }
    }
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    edits.dedup_by_key(|e| e.0);
    let mut out = text.to_string();
    let restored = edits.len();
    for (at, replaced, insert) in edits {
        out.replace_range(at..at + replaced, &insert);
    }
    (out, restored)
}

/// Repone en el Markdown (con marcas `<!-- page: N -->` en su propio bloque) las rayas que el PDF
/// muestra y el lector perdió. `page_texts` es el texto de cada página según PDFium
/// (`pdf_styles::page_texts`); en un escaneo llega vacío y no cambia nada.
pub fn restore_dashes(markdown: &str, page_texts: &HashMap<u32, String>) -> (String, usize) {
    let by_page: HashMap<u32, Vec<DashPair>> = page_texts
        .iter()
        .map(|(page, text)| (*page, pairs(text)))
        .filter(|(_, p)| !p.is_empty())
        .collect();
    if by_page.is_empty() {
        return (markdown.to_string(), 0);
    }
    let mut out = String::with_capacity(markdown.len() + 64);
    let mut restored = 0;
    let mut page: Option<u32> = None;
    let mut rest = markdown;
    loop {
        let next = rest.find("<!-- page:");
        let (segment, tail) = match next {
            Some(at) => rest.split_at(at),
            None => (rest, ""),
        };
        match page.and_then(|p| by_page.get(&p)) {
            Some(page_pairs) => {
                let (fixed, count) = restore_page(segment, page_pairs);
                out.push_str(&fixed);
                restored += count;
            }
            None => out.push_str(segment),
        }
        if tail.is_empty() {
            break;
        }
        let end = tail.find("-->").map_or(tail.len(), |e| e + 3);
        page = tail["<!-- page:".len()..end.saturating_sub(3)]
            .trim()
            .parse()
            .ok();
        out.push_str(&tail[..end]);
        rest = &tail[end..];
    }
    (out, restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(page: u32, texts: &[&str]) -> HashMap<u32, String> {
        HashMap::from([(page, texts.join("\n"))])
    }

    fn doc(body: &str) -> String {
        format!("<!-- page: 1 -->\n\n{body}\n\n<!-- page: 2 -->\n\nOtra página.")
    }

    #[test]
    fn glued_words_get_their_dash_back() {
        let pdf = lines(1, &["We live in the flicker—may it last as long"]);
        let (out, n) = restore_dashes(&doc("We live in the flickermay it last as long"), &pdf);
        assert_eq!(n, 1);
        assert!(out.contains("flicker—may it last"));
    }

    #[test]
    fn dash_at_line_end_replaces_the_reader_space() {
        let pdf = lines(
            1,
            &["Light came out of this river since—", "you say Knights?"],
        );
        let (out, n) = restore_dashes(&doc("river since you say Knights?"), &pdf);
        assert_eq!(n, 1);
        assert!(out.contains("since—you say"));
    }

    #[test]
    fn paragraph_break_after_the_dash_is_kept() {
        let pdf = lines(1, &["and presently he said, very slow—", "“I was thinking"]);
        let (out, _) = restore_dashes(&doc("he said, very slow\n\n“I was thinking"), &pdf);
        assert!(out.contains("very slow—\n\n“I was"));
    }

    #[test]
    fn spaced_style_keeps_its_spaces() {
        let pdf = lines(1, &["el artículo – que se cita"]);
        let (out, _) = restore_dashes(&doc("el artículoque se cita"), &pdf);
        assert!(out.contains("artículo – que"));
    }

    #[test]
    fn present_dashes_and_other_pages_are_untouched() {
        let pdf = lines(1, &["years ago—the other day"]);
        let markdown = doc("years ago— the other day");
        assert_eq!(restore_dashes(&markdown, &pdf), (markdown.clone(), 0));
        // La raya está en la página 1 del PDF: el texto de la página 2 no se toca.
        let markdown = "<!-- page: 1 -->\n\nNada.\n\n<!-- page: 2 -->\n\nyears agothe";
        assert_eq!(restore_dashes(markdown, &pdf).1, 0);
    }

    #[test]
    fn ambiguous_pairs_are_left_alone() {
        // «some thing» aparece dos veces sin raya y solo falta una: no se sabe cuál.
        let pdf = lines(1, &["some—thing here"]);
        let (out, n) = restore_dashes(&doc("some thing here and some thing there"), &pdf);
        assert_eq!(n, 0);
        assert!(!out.contains('—'));
        // Una palabra real que coincide con el par pegado tampoco se parte.
        let pdf = lines(1, &["no—body said"]);
        assert_eq!(restore_dashes(&doc("nobody said, nobody came"), &pdf).1, 0);
    }

    #[test]
    fn no_page_text_means_no_change() {
        let markdown = doc("flickermay");
        assert_eq!(
            restore_dashes(&markdown, &HashMap::new()),
            (markdown.clone(), 0)
        );
    }

    #[test]
    fn dialogue_and_double_dashes_are_not_pairs() {
        assert!(pairs("—Hola, dijo.\nyarns——and").is_empty());
    }
}
