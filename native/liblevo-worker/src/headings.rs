//! Detección general de títulos a partir del estilo observado en el PDF.
//!
//! Complementa a un lector que conserva bien el texto pero no marca los
//! títulos. Usa solo señales tipográficas y de composición observadas en el propio documento
//! (tamaño respecto al cuerpo, negrita de toda la línea, longitud y aislamiento); nunca palabras,
//! patrones o nombres de un documento concreto. Lo desconocido no se convierte en título.

use std::collections::{BTreeMap, HashMap};

/// Una línea del PDF con su estilo dominante.
#[derive(Debug, Clone, PartialEq)]
pub struct StyledLine {
    pub page: u32,
    pub text: String,
    /// Tamaño de letra dominante, en puntos.
    pub size: f64,
    /// Toda la línea (al menos el 80 % de sus letras) está en una fuente negrita.
    pub bold: bool,
    /// Distancia vertical desde la línea anterior de la misma página (`None` si es la primera).
    pub gap_before: Option<f64>,
    /// La línea está centrada respecto a la columna de texto y es claramente más estrecha.
    pub centered: bool,
}

/// Un marcador del PDF: título, página de destino (desde 1) y nivel de anidamiento (desde 0).
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineEntry {
    pub title: String,
    pub page: u32,
    pub depth: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Heading {
    pub text: String,
    pub level: u8,
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn margin_key(text: &str) -> String {
    let lowered: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect();
    lowered.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Tamaño del cuerpo: el más frecuente, ponderado por letras y redondeado a medio punto.
pub fn body_size(lines: &[StyledLine]) -> f64 {
    let mut weight: HashMap<i64, usize> = HashMap::new();
    for line in lines {
        *weight.entry((line.size * 2.0).round() as i64).or_default() += line.text.chars().count();
    }
    weight
        .into_iter()
        .max_by_key(|(size, count)| (*count, -*size))
        .map_or(10.0, |(size, _)| size as f64 / 2.0)
}

/// Propone títulos. Las líneas consecutivas con el mismo estilo de título se unen.
/// Propone títulos. Los marcadores del PDF, si existen, también marcan como título la línea corta
/// de su página de destino cuyo texto coincide con el marcador o con su final («I» en
/// «Chapter I»); su nivel sale del anidamiento del marcador.
pub fn propose(lines: &[StyledLine], outline: &[OutlineEntry]) -> Vec<Heading> {
    let body = body_size(lines);
    // Texto repetido en muchas páginas (cabeceras corridas) no es un título.
    // Cabeceras corridas: el mismo texto (cambiando números) en lo alto o al pie de al menos una
    // cuarta parte de las páginas. Una serie como «ANEXO 1», «ANEXO 2» no lo es.
    let mut by_page_lines: BTreeMap<u32, Vec<&StyledLine>> = BTreeMap::new();
    for line in lines {
        by_page_lines.entry(line.page).or_default().push(line);
    }
    let mut pages_with: HashMap<String, std::collections::BTreeSet<u32>> = HashMap::new();
    for (page, page_lines) in &by_page_lines {
        let n = page_lines.len();
        for (i, line) in page_lines.iter().enumerate() {
            if i < 2 || i + 2 >= n {
                pages_with
                    .entry(margin_key(&line.text))
                    .or_default()
                    .insert(*page);
            }
        }
    }
    let running_threshold = 3.max(by_page_lines.len().div_ceil(4));
    let is_running = |text: &str| {
        pages_with
            .get(&margin_key(text))
            .is_some_and(|pages| pages.len() >= running_threshold)
    };
    let is_candidate = |line: &StyledLine| {
        let text = line.text.trim();
        let count = words(text).len();
        let larger = line.size >= body * 1.12;
        let bold_body = line.bold && line.size >= body * 0.95;
        let centered_body = line.centered && line.size >= body * 0.95;
        (1..=15).contains(&count)
            && text.chars().any(char::is_alphabetic)
            && !text.ends_with([',', ';'])
            && !is_running(text)
            && (larger || bold_body || centered_body)
    };
    // Estilos de título ordenados de mayor a menor: tamaño y, a igual tamaño, negrita primero.
    let style = |line: &StyledLine| ((line.size * 2.0).round() as i64, line.bold, line.centered);
    let mut styles: Vec<(i64, bool, bool)> = lines
        .iter()
        .filter(|l| is_candidate(l))
        .map(style)
        .collect();
    styles.sort_by(|a, b| b.cmp(a));
    styles.dedup();
    let levels: BTreeMap<(i64, bool, bool), u8> = styles
        .iter()
        .enumerate()
        .map(|(i, s)| (*s, (i as u8 + 1).min(4)))
        .collect();

    let mut by_page: HashMap<u32, Vec<(Vec<String>, u8)>> = HashMap::new();
    for entry in outline {
        let title = words(&entry.title);
        if !title.is_empty() {
            by_page
                .entry(entry.page)
                .or_default()
                .push((title, (entry.depth + 1).min(4)));
        }
    }
    let mut position_in_page: HashMap<u32, usize> = HashMap::new();
    let outline_level = |line: &StyledLine, position: usize| -> Option<u8> {
        let found = words(&line.text);
        if found.is_empty() || found.len() > 15 || position >= 8 {
            return None;
        }
        by_page.get(&line.page)?.iter().find_map(|(title, level)| {
            // Coincidir solo con el final del marcador exige además una señal visual: un párrafo
            // que empieza por «I» no es el capítulo «I».
            let distinct = line.centered || line.bold || line.size >= body * 1.12;
            let ends = distinct && found.len() <= 4 && title.ends_with(&found);
            (title == &found || ends).then_some(*level)
        })
    };

    let mut headings: Vec<Heading> = Vec::new();
    let mut previous: Option<&StyledLine> = None;
    for line in lines {
        let position = {
            let counter = position_in_page.entry(line.page).or_default();
            *counter += 1;
            *counter - 1
        };
        let from_outline = outline_level(line, position);
        if !is_candidate(line) && from_outline.is_none() {
            previous = Some(line);
            continue;
        }
        if let Some(level) = from_outline {
            headings.push(Heading {
                text: line.text.trim().to_string(),
                level,
            });
            previous = None;
            continue;
        }
        let level = levels[&style(line)];
        let continues = previous.is_some_and(|p| {
            p.page == line.page
                && is_candidate(p)
                && style(p) == style(line)
                && line.gap_before.is_some_and(|g| g <= line.size * 1.6)
        });
        match headings.last_mut() {
            Some(last) if continues => {
                last.text.push(' ');
                last.text.push_str(line.text.trim());
            }
            _ => headings.push(Heading {
                text: line.text.trim().to_string(),
                level,
            }),
        }
        previous = Some(line);
    }
    headings
}

/// Marca en el Markdown los párrafos que coinciden con un título propuesto. Un párrafo que
/// empieza por el título se divide; los títulos que ya existían se conservan.
pub fn apply(markdown: &str, headings: &[Heading]) -> String {
    let markdown = markdown.replace("\r\n", "\n");
    let mut blocks: Vec<String> = markdown.split("\n\n").map(str::to_string).collect();
    // Se comparan solo letras y cifras: el PDF y el lector pueden espaciar distinto un título
    // («H EA RT» frente a «HEART»).
    let matches = |block: &str, target: &str| {
        let block = block.trim();
        if block.starts_with('#') || block.starts_with("<!--") {
            return false;
        }
        let found = letters(block);
        // Un título muy corto («I», «2») solo se marca si ocupa todo el bloque: nunca parte un
        // párrafo que casualmente empieza igual.
        if target.chars().count() <= 3 {
            return found == target;
        }
        found.starts_with(target)
    };
    let mut start = 0;
    for heading in headings {
        let target = letters(&heading.text);
        if target.is_empty() {
            continue;
        }
        // Primero hacia delante; si el lector lo puso en otro orden, también hacia atrás.
        let Some(index) = (start..blocks.len())
            .chain(0..start.min(blocks.len()))
            .find(|&i| matches(&blocks[i], &target))
        else {
            continue;
        };
        let block = blocks[index].trim().to_string();
        let marker = "#".repeat(usize::from(heading.level));
        // Solo el texto del título pierde las marcas de énfasis; el resto queda intacto.
        let cut = cut_after_letters(&block, target.chars().count());
        let title = block[..cut].replace(['*', '`'], "");
        let rest = block[cut..]
            .trim_start_matches(['*', '`'])
            .trim()
            .to_string();
        blocks[index] = format!("{marker} {}", title.trim());
        if !rest.is_empty() {
            // El lector unió el título con el texto siguiente: el resto queda como bloque propio,
            // que puede ser a su vez otro título.
            blocks.insert(index + 1, rest);
        }
        start = index + 1;
    }
    blocks.join("\n\n")
}

/// Letras y cifras en minúsculas, sin espacios ni signos.
fn letters(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Posición en bytes justo después de la letra o cifra número `count`, incluido el resto de la
/// palabra en la que cae y los signos que la cierran (un punto, un paréntesis).
fn cut_after_letters(text: &str, count: usize) -> usize {
    let mut seen = 0;
    for (i, c) in text.char_indices() {
        if c.is_alphanumeric() {
            seen += 1;
            if seen == count {
                let rest = &text[i + c.len_utf8()..];
                let word_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                return i + c.len_utf8() + word_end;
            }
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(page: u32, text: &str, size: f64, bold: bool) -> StyledLine {
        StyledLine {
            page,
            text: text.into(),
            size,
            bold,
            gap_before: Some(14.0),
            centered: false,
        }
    }

    fn body(page: u32, n: usize) -> Vec<StyledLine> {
        (0..n)
            .map(|i| {
                line(
                    page,
                    &format!("cuerpo de texto corriente número {i} con varias palabras"),
                    10.0,
                    false,
                )
            })
            .collect()
    }

    #[test]
    fn outline_entries_mark_their_printed_lines() {
        let mut lines = body(1, 5);
        let mut chapter = line(2, "I", 10.0, false);
        chapter.centered = true;
        lines.push(chapter);
        lines.extend(body(2, 20));
        lines.push(line(3, "Copyright Information", 10.0, false));
        lines.extend(body(3, 5));
        let outline = vec![
            OutlineEntry {
                title: "Chapter I".into(),
                page: 2,
                depth: 0,
            },
            OutlineEntry {
                title: "Copyright Information".into(),
                page: 3,
                depth: 1,
            },
            OutlineEntry {
                title: "Missing".into(),
                page: 1,
                depth: 0,
            },
        ];
        assert_eq!(
            propose(&lines, &outline),
            vec![
                Heading {
                    text: "I".into(),
                    level: 1
                },
                Heading {
                    text: "Copyright Information".into(),
                    level: 2
                },
            ]
        );
    }

    #[test]
    fn short_titles_never_split_a_paragraph() {
        let markdown = "I was thinking of very old times.

I

The chapter starts.";
        let headings = vec![Heading {
            text: "I".into(),
            level: 2,
        }];
        assert_eq!(
            apply(markdown, &headings),
            "I was thinking of very old times.

## I

The chapter starts."
        );
    }

    #[test]
    fn outline_suffix_needs_a_visual_signal() {
        let mut lines = body(2, 20);
        lines.insert(0, line(2, "I was", 10.0, false));
        let outline = vec![OutlineEntry {
            title: "Chapter I was".into(),
            page: 2,
            depth: 0,
        }];
        assert!(propose(&lines, &outline).is_empty());
    }

    #[test]
    fn a_numbered_series_is_not_a_running_header() {
        let mut lines = Vec::new();
        for page in 1..=12 {
            lines.extend(body(page, 8));
            if page % 3 == 0 {
                let mut annex = line(page, &format!("ANEXO {}", page / 3), 10.0, true);
                annex.centered = true;
                lines.insert(lines.len() - 4, annex);
            }
        }
        let found: Vec<String> = propose(&lines, &[]).into_iter().map(|h| h.text).collect();
        assert_eq!(found, vec!["ANEXO 1", "ANEXO 2", "ANEXO 3", "ANEXO 4"]);
    }

    #[test]
    fn larger_and_bold_isolated_lines_become_ordered_headings() {
        let mut lines = vec![line(1, "Un título grande", 18.0, false)];
        lines.extend(body(1, 20));
        lines.push(line(1, "Apartado en negrita", 10.0, true));
        lines.extend(body(1, 20));
        let headings = propose(&lines, &[]);
        assert_eq!(
            headings,
            vec![
                Heading {
                    text: "Un título grande".into(),
                    level: 1
                },
                Heading {
                    text: "Apartado en negrita".into(),
                    level: 2
                },
            ]
        );
    }

    #[test]
    fn body_text_long_lines_and_running_headers_are_not_headings() {
        let mut lines = Vec::new();
        for page in 1..=4 {
            lines.push(line(page, &format!("REVISTA DE PRUEBA {page}"), 12.0, true));
            lines.extend(body(page, 10));
        }
        lines.push(line(
            2,
            "una línea en negrita demasiado larga para ser un título porque sigue y sigue con muchas palabras",
            10.0,
            true,
        ));
        lines.push(line(3, "Termina en coma,", 14.0, false));
        assert!(propose(&lines, &[]).is_empty());
    }

    #[test]
    fn short_centered_lines_at_body_size_are_headings() {
        let mut lines = body(1, 10);
        let mut chapter = line(1, "CAPÍTULO II", 10.0, false);
        chapter.centered = true;
        lines.push(chapter);
        lines.extend(body(1, 10));
        assert_eq!(
            propose(&lines, &[]),
            vec![Heading {
                text: "CAPÍTULO II".into(),
                level: 1
            }]
        );
    }

    #[test]
    fn consecutive_lines_with_the_same_heading_style_are_joined() {
        let mut lines = body(1, 10);
        lines.push(line(1, "Título que ocupa", 16.0, false));
        lines.push(line(1, "dos líneas", 16.0, false));
        lines.extend(body(1, 10));
        assert_eq!(propose(&lines, &[])[0].text, "Título que ocupa dos líneas");
    }

    #[test]
    fn merged_titles_and_out_of_order_titles_are_all_marked() {
        let markdown = "*Sección 1.ª Ámbito* TÍTULO PRELIMINAR\n\nTexto.\n\nANEXO 1 Grupos";
        let headings = vec![
            Heading {
                text: "ANEXO 1".into(),
                level: 2,
            },
            Heading {
                text: "Sección 1.ª Ámbito".into(),
                level: 3,
            },
            Heading {
                text: "TÍTULO PRELIMINAR".into(),
                level: 2,
            },
        ];
        assert_eq!(
            apply(markdown, &headings),
            "### Sección 1.ª Ámbito\n\n## TÍTULO PRELIMINAR\n\nTexto.\n\n## ANEXO 1\n\nGrupos"
        );
    }

    #[test]
    fn apply_keeps_emphasis_and_underscores_of_the_following_text() {
        let markdown = "*Breve título* el valor *mayor* de snake_case sigue.";
        let headings = vec![Heading {
            text: "Breve título".into(),
            level: 2,
        }];
        assert_eq!(
            apply(markdown, &headings),
            "## Breve título

el valor *mayor* de snake_case sigue."
        );
    }

    #[test]
    fn apply_marks_exact_and_merged_paragraphs_without_losing_text() {
        // Saltos de línea de Windows mezclados con los de Unix, como los que escriben algunos lectores.
        let markdown = "<!-- page: 1 -->\r\n\r\nCAPÍTULO I\r\n\r\n*Sección 1.ª Ámbito* Texto que sigue.\n\n# Ya era título";
        let headings = vec![
            Heading {
                text: "CAPÍTULO I".into(),
                level: 2,
            },
            Heading {
                text: "Sección 1.ª Ámbito".into(),
                level: 3,
            },
            Heading {
                text: "Ya era título".into(),
                level: 1,
            },
        ];
        let out = apply(markdown, &headings);
        assert!(out.contains("\n\n## CAPÍTULO I\n\n"));
        assert!(out.contains("### Sección 1.ª Ámbito\n\nTexto que sigue."));
        assert!(out.ends_with("# Ya era título"));
    }
}
