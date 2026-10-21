//! Limpieza general del Markdown de un lector: cabeceras y pies repetidos y guiones de fin de
//! línea. Usa solo pruebas del propio documento (repetición entre páginas, otras apariciones de
//! una palabra); nunca diccionarios, listas de palabras ni reglas de un documento concreto.
//!
//! El Markdown lleva una marca `<!-- page: N -->` al empezar cada página.

use std::collections::{HashMap, HashSet};

const PAGE_MARKER: &str = "<!-- page:";

fn is_marker(block: &str) -> bool {
    block.trim_start().starts_with(PAGE_MARKER)
}

fn key(block: &str) -> String {
    let lowered: String = block
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '#' || c.is_whitespace())
        .collect();
    let mut key = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    while key.contains("##") {
        key = key.replace("##", "#");
    }
    key
}

/// Páginas mínimas de una racha de cabecera de parte o de capítulo.
const MIN_RUN_PAGES: usize = 5;
/// Distancia máxima entre dos apariciones seguidas de una racha (cabeceras en páginas alternas).
const MAX_RUN_GAP: usize = 2;

/// La racha más larga de páginas en las que aparece una clave, sin saltar más de `MAX_RUN_GAP`.
fn longest_run(pages: &[usize]) -> usize {
    let mut best = 0;
    let mut run = 0;
    let mut previous: Option<usize> = None;
    for &page in pages {
        run = match previous {
            Some(p) if page - p <= MAX_RUN_GAP => run + 1,
            _ => 1,
        };
        best = best.max(run);
        previous = Some(page);
    }
    best
}

/// Quita bloques cortos que se repiten al principio o al final de las páginas (cabeceras, pies,
/// números de página): en muchas páginas del documento, o en una racha de páginas seguidas o
/// alternas, como la cabecera de una parte o de un capítulo. Un título que aparece en páginas
/// sueltas no forma racha. Devuelve el Markdown y cuántos bloques se quitaron.
pub fn remove_running_matter(markdown: &str) -> (String, usize) {
    let markdown = markdown.replace("\r\n", "\n");
    let blocks: Vec<&str> = markdown.split("\n\n").collect();
    // Índices de los bloques de contenido de cada página.
    let mut pages: Vec<Vec<usize>> = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        if is_marker(block) {
            pages.push(Vec::new());
        } else if !block.trim().is_empty()
            && let Some(page) = pages.last_mut()
        {
            page.push(index);
        }
    }
    if pages.len() < 3 {
        return (markdown, 0);
    }
    let edge = |page: &[usize]| -> Vec<usize> {
        let mut chosen: Vec<usize> = page.iter().take(2).copied().collect();
        chosen.extend(page.iter().rev().take(2).copied());
        chosen.sort_unstable();
        chosen.dedup();
        chosen
    };
    let short = |block: &str| block.split_whitespace().count() <= 8 && !block.contains('|');
    let mut pages_with: HashMap<String, Vec<usize>> = HashMap::new();
    for (number, page) in pages.iter().enumerate() {
        let keys: HashSet<String> = edge(page)
            .into_iter()
            .filter(|&i| short(blocks[i]))
            .map(|i| key(blocks[i]))
            .filter(|k| !k.is_empty())
            .collect();
        for k in keys {
            pages_with.entry(k).or_default().push(number);
        }
    }
    let threshold = 3.max((pages.len() as f64 * 0.15).ceil() as usize);
    let repeated: HashSet<&String> = pages_with
        .iter()
        .filter(|(_, found)| found.len() >= threshold || longest_run(found) >= MIN_RUN_PAGES)
        .map(|(k, _)| k)
        .collect();
    let mut remove: HashSet<usize> = HashSet::new();
    for page in &pages {
        for index in edge(page) {
            if short(blocks[index]) && repeated.contains(&key(blocks[index])) {
                remove.insert(index);
            }
        }
    }
    let kept: Vec<&str> = blocks
        .iter()
        .enumerate()
        .filter(|(i, _)| !remove.contains(i))
        .map(|(_, b)| *b)
        .collect();
    (kept.join("\n\n"), remove.len())
}

/// Caracteres como máximo de un adorno (separadores de sección, viñetas sueltas).
const MAX_ORNAMENT_CHARS: usize = 3;

/// Quita los bloques que son solo un adorno: de uno a tres símbolos, sin letras ni cifras latinas,
/// que no existen en el texto de esa página del PDF (`page_texts`). Es el caso de un dibujo de
/// separación que el lector interpreta como un carácter. Devuelve el Markdown y cuántos quitó.
pub fn remove_foreign_ornaments(
    markdown: &str,
    page_texts: &HashMap<u32, String>,
) -> (String, usize) {
    if page_texts.is_empty() {
        return (markdown.to_string(), 0);
    }
    let mut page: Option<u32> = None;
    let mut removed = 0;
    let kept: Vec<&str> = markdown
        .split("\n\n")
        .filter(|block| {
            if let Some(rest) = block.trim().strip_prefix(PAGE_MARKER) {
                page = rest.trim_end_matches("-->").trim().parse().ok();
                return true;
            }
            let symbols: Vec<char> = block.chars().filter(|c| !c.is_whitespace()).collect();
            let ornament = !symbols.is_empty()
                && symbols.len() <= MAX_ORNAMENT_CHARS
                && !symbols
                    .iter()
                    .any(|c| c.is_ascii_alphanumeric() || "![]()|#*_".contains(*c));
            let in_pdf = page
                .and_then(|p| page_texts.get(&p))
                .is_some_and(|text| symbols.iter().any(|c| text.contains(*c)));
            let drop = ornament && page.is_some() && !in_pdf;
            removed += usize::from(drop);
            !drop
        })
        .collect();
    (kept.join("\n\n"), removed)
}

fn words_of(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
}

/// Une las palabras partidas por un guion de fin de línea. Mantiene el guion cuando el propio
/// documento muestra que es un compuesto. Devuelve el Markdown y cuántas uniones se hicieron.
pub fn join_hyphenated_words(markdown: &str) -> (String, usize) {
    let text = markdown.replace("\r\n", "\n");
    let mut counts: HashMap<String, usize> = HashMap::new();
    for word in words_of(&text) {
        *counts.entry(word).or_default() += 1;
    }
    let seen = |w: &str| counts.get(w).copied().unwrap_or(0);
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut joined = 0;
    let mut i = 0;
    let mut in_fence = false;
    let mut line_start = true;
    let mut table_line = false;
    while i < chars.len() {
        let c = chars[i];
        if line_start {
            let rest: String = chars[i..chars.len().min(i + 3)].iter().collect();
            if rest == "```" {
                in_fence = !in_fence;
            }
            table_line = c == '|';
        }
        line_start = c == '\n';
        if c == '-' && !in_fence && !table_line && i > 0 && chars[i - 1].is_lowercase() {
            // Separación: espacios, saltos de línea y marcas de página entre las dos mitades.
            let mut j = i + 1;
            let mut gap = String::new();
            let mut crossed = false;
            loop {
                while j < chars.len() && chars[j].is_whitespace() {
                    crossed |= chars[j] == '\n' || chars[j] == ' ';
                    j += 1;
                }
                let tail: String = chars[j..chars.len().min(j + PAGE_MARKER.len())]
                    .iter()
                    .collect();
                if tail == PAGE_MARKER {
                    let end = (j..chars.len().saturating_sub(2))
                        .find(|&k| chars[k] == '-' && chars[k + 1] == '-' && chars[k + 2] == '>')
                        .map(|k| k + 3);
                    let Some(end) = end else { break };
                    gap.push_str(&chars[j..end].iter().collect::<String>());
                    j = end;
                    continue;
                }
                break;
            }
            if crossed && j < chars.len() && chars[j].is_lowercase() {
                let start = (0..i)
                    .rev()
                    .take_while(|&k| chars[k].is_alphabetic())
                    .last()
                    .unwrap_or(i);
                let end = (j..chars.len())
                    .take_while(|&k| chars[k].is_alphabetic())
                    .last()
                    .map_or(j, |k| k + 1);
                let left: String = chars[start..i].iter().collect();
                let right: String = chars[j..end].iter().collect();
                if left.chars().count() >= 2 && !right.is_empty() {
                    let (l, r) = (left.to_lowercase(), right.to_lowercase());
                    let compound = seen(&format!("{l}-{r}")) > 0
                        || (seen(&format!("{l}{r}")) == 0
                            && l.chars().count() >= 3
                            && r.chars().count() >= 3
                            && seen(&l) > 0
                            && seen(&r) > 0);
                    if compound {
                        out.push('-');
                    }
                    out.push_str(&right);
                    // Una marca de página que había en medio se coloca tras la palabra.
                    let mut next = end;
                    if !gap.is_empty() {
                        out.push_str("\n\n");
                        out.push_str(gap.trim());
                        out.push_str("\n\n");
                        while next < chars.len() && chars[next] == ' ' {
                            next += 1;
                        }
                    }
                    joined += 1;
                    i = next;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    (out, joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_headers_page_numbers_and_footers_are_removed() {
        let words = ["alfa", "beta", "gamma", "delta", "épsilon", "zeta", "eta"];
        let mut doc = String::new();
        for page in 1..=6 {
            let (a, b) = (words[page], words[(page + 3) % 7]);
            doc.push_str(&format!(
                "<!-- page: {page} -->\n\nREVISTA DE PRUEBA\n\nTexto único de la página {page} con contenido propio y largo.\n\nOtro párrafo sobre {a} y {b}.\n\n{page}\n\n"
            ));
        }
        let (clean, removed) = remove_running_matter(&doc);
        assert_eq!(removed, 12);
        assert!(!clean.contains("REVISTA"));
        assert!(clean.contains("Texto único de la página 3"));
        assert!(clean.contains("<!-- page: 6 -->"));
    }

    #[test]
    fn part_headers_in_a_run_are_removed_but_scattered_titles_stay() {
        // Libro de 60 páginas: «Part One» solo en las pares de la 2 a la 12 (menos del 15 %).
        let mut doc = String::new();
        for page in 1..=60 {
            doc.push_str(&format!(
                "<!-- page: {page} -->

Texto {page} del cuerpo de la página, con bastantes más de ocho palabras en total.

"
            ));
            if page % 2 == 0 && page <= 12 {
                doc.push_str(&format!(
                    "{page} Part One

"
                ));
            }
            // Un título repetido en páginas sueltas (cada diez) no es una racha.
            if page % 10 == 1 {
                doc.push_str(
                    "ANEXO

",
                );
            }
        }
        let (out, removed) = remove_running_matter(&doc);
        assert_eq!(removed, 6);
        assert!(!out.contains("Part One"));
        assert_eq!(out.matches("ANEXO").count(), 6);
        assert_eq!(longest_run(&[1, 3, 5, 6, 8, 20]), 5);
        assert_eq!(longest_run(&[1, 10, 20]), 1);
    }

    #[test]
    fn ornaments_missing_from_the_pdf_are_removed() {
        let doc = "<!-- page: 1 -->\n\nFin de sección.\n\n\u{6728}\n\n§\n\n—\n\n10\n\n\
            <!-- page: 2 -->\n\n\u{6728}";
        let texts = HashMap::from([
            (1, "Fin de sección.\n§".to_string()),
            (2, "\u{6728}".to_string()),
        ]);
        let (out, removed) = remove_foreign_ornaments(doc, &texts);
        // Se van el carácter que el PDF no tiene y la raya suelta; se quedan «§» (está en el PDF),
        // el número y el carácter de la página 2, que sí está en su texto.
        assert_eq!(removed, 2);
        assert_eq!(
            out,
            "<!-- page: 1 -->\n\nFin de sección.\n\n§\n\n10\n\n<!-- page: 2 -->\n\n\u{6728}"
        );
        // Sin texto del PDF (escaneos) no se toca nada.
        assert_eq!(remove_foreign_ornaments(doc, &HashMap::new()).1, 0);
    }

    #[test]
    fn few_pages_or_unique_edges_are_left_alone() {
        let doc =
            "<!-- page: 1 -->\n\nTítulo\n\nTexto.\n\n<!-- page: 2 -->\n\nTítulo\n\nMás texto.";
        assert_eq!(remove_running_matter(doc), (doc.to_string(), 0));
    }

    #[test]
    fn syllable_breaks_are_joined_and_compounds_keep_their_hyphen() {
        let doc = "He ex- plained it. The stepping- stones were wet; stepping and stones. \
                   A long- term plan and another long-term idea. Con-\nstant noise.";
        let (clean, joined) = join_hyphenated_words(doc);
        assert!(clean.contains("He explained it."));
        assert!(clean.contains("The stepping-stones were wet"));
        assert!(clean.contains("A long-term plan"));
        assert!(clean.contains("Constant noise") || clean.contains("Con-\nstant"));
        assert!(joined >= 3);
    }

    #[test]
    fn hyphens_across_a_page_break_keep_the_marker_after_the_word() {
        let doc = "the acquain-\n\n<!-- page: 7 -->\n\ntance was brief";
        let (clean, joined) = join_hyphenated_words(doc);
        assert_eq!(joined, 1);
        assert_eq!(clean, "the acquaintance\n\n<!-- page: 7 -->\n\nwas brief");
    }

    #[test]
    fn real_hyphens_tables_code_and_capitals_are_untouched() {
        let doc = "well-known fact\n\n| a- b | c |\n\n```\nx- y\n```\n\nNew- York";
        let (clean, joined) = join_hyphenated_words(doc);
        assert_eq!(joined, 0);
        assert_eq!(clean, doc);
    }
}
