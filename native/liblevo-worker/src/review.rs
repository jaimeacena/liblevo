//! Lista de revisión asistida: los fragmentos del resultado que conviene mirar, ordenados por
//! riesgo. Usa las mismas señales que la conversión y ninguna regla de un documento concreto:
//!
//! - palabras del resultado que no aparecen en el texto de esa página del PDF (en un PDF digital,
//!   su texto; en un escaneo, su capa invisible, si la tiene) ni en las páginas vecinas;
//! - tokens raros: letras y cifras mezcladas dentro de una palabra, el carácter de sustitución o
//!   letras de otra escritura distinta de la del documento;
//! - el estado de la página: dudosa sin releer, relectura rechazada o releída por el modelo.
//!
//! Ninguna ausencia de avisos certifica que un fragmento sea correcto; la lista solo ordena qué
//! mirar primero.

use crate::doubt::markdown_pages;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

/// Ejemplos que se guardan de cada señal (los justos para entender el aviso).
const MAX_EXAMPLES: usize = 5;
/// Palabras ajenas que cuentan como máximo en un fragmento (un párrafo largo no ahoga al resto).
const MAX_UNSUPPORTED: usize = 10;
/// Terminaciones de ordinales y abreviaturas numéricas habituales («19th», «2º», «1er», «3a»).
const ORDINAL_SUFFIXES: [&str; 18] = [
    "st", "nd", "rd", "th", "d", "o", "a", "er", "e", "re", "eme", "ème", "º", "ª", "os", "as",
    "do", "to",
];
/// Guiones de fin de línea en el texto de PDFium (incluido el de control con el que marca el guion
/// de partición).
const HYPHENS: [char; 6] = [
    '-', '\u{00AD}', '\u{0002}', '\u{2010}', '\u{2011}', '\u{FFFE}',
];
/// Marcas de partición de PDFium: unen la palabra aunque no les siga un salto de línea.
const PARTITION_MARKS: [char; 3] = ['\u{00AD}', '\u{0002}', '\u{FFFE}'];
/// Apariciones a partir de las cuales una palabra es frecuente en el documento.
const FREQUENT: usize = 3;
/// Fragmentos con el mismo token raro a partir de los cuales es sistemático (un código, una marca
/// de imprenta), no un error suelto.
const SYSTEMATIC: usize = 3;
/// Letras finas que el OCR pierde o añade a menudo.
const THIN: &str = "iljtrf";
/// Pares de letras que el OCR confunde a menudo.
const CONFUSABLE: [(char, char); 10] = [
    ('c', 'e'),
    ('e', 'o'),
    ('n', 'u'),
    ('h', 'b'),
    ('i', 'l'),
    ('l', 't'),
    ('f', 't'),
    ('a', 'o'),
    ('s', 'a'),
    ('r', 'n'),
];
/// Caracteres del extracto que se muestra.
const EXCERPT_CHARS: usize = 180;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reason {
    /// Palabras que el texto de la página del PDF no contiene.
    UnsupportedWords { count: usize, examples: Vec<String> },
    /// Tokens con forma rara o símbolos que el PDF no tiene.
    OddTokens { count: usize, examples: Vec<String> },
    /// Palabras que el propio documento sugiere mal leídas: únicas y casi iguales a una frecuente
    /// («inereased» frente a «increased»), o trozos que juntos forman una frecuente («qu ite»).
    SuspectWords { count: usize, examples: Vec<String> },
    /// Página dudosa que no se releyó (sin modelo visual o sin tiempo).
    DoubtfulPage,
    /// Página releída por el modelo visual: su texto viene de él.
    VisualReread,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub page: u32,
    /// Posición del fragmento entre los fragmentos de su página (0 = primero); `None` para un aviso
    /// de página entera.
    pub block: Option<usize>,
    /// Huella corta del texto del fragmento, para reconocerlo aunque cambie de sitio.
    pub key: String,
    pub excerpt: String,
    pub risk: f64,
    pub reasons: Vec<Reason>,
}

/// Estado de las páginas tras la conversión.
#[derive(Debug, Clone, Default)]
pub struct PageStates {
    pub doubtful: Vec<u32>,
    pub reread: Vec<u32>,
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .map(str::to_lowercase)
        .collect()
}

/// Palabras de un texto de referencia, incluidas las partidas por un guion de fin de línea.
fn reference_words(text: &str) -> HashSet<String> {
    let mut set: HashSet<String> = words(text).into_iter().collect();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if !HYPHENS.contains(&c) {
            continue;
        }
        let mut j = i + 1;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        let line_break = chars[i + 1..j].contains(&'\n');
        if !line_break && !PARTITION_MARKS.contains(&c) {
            continue;
        }
        let mut start = i;
        while start > 0 && chars[start - 1].is_alphanumeric() {
            start -= 1;
        }
        let mut end = j;
        while end < chars.len() && chars[end].is_alphanumeric() {
            end += 1;
        }
        let joined: String = chars[start..i].iter().chain(&chars[j..end]).collect();
        set.insert(joined.to_lowercase());
    }
    set
}

/// Texto de referencia de una página. Una palabra cuenta como presente si es una de sus palabras o
/// si el PDF la pegó a otra palabra real («allrealms» contiene «realms» porque «all» existe): hay PDF
/// cuyo texto interno pega palabras. Un trozo cuyo resto no es una palabra («mpously» en
/// «pompously») no cuenta, para no esconder las palabras partidas por el lector.
struct Reference {
    words: HashSet<String>,
}

impl Reference {
    fn new(text: &str) -> Self {
        Self {
            words: reference_words(text),
        }
    }

    fn has(&self, word: &str, is_word: impl Fn(&str) -> bool) -> bool {
        self.words.contains(word)
            || self.words.iter().any(|token| {
                token.len() > word.len()
                    && (token.strip_prefix(word).is_some_and(&is_word)
                        || token.strip_suffix(word).is_some_and(&is_word))
            })
    }
}

/// Escritura principal de un carácter alfabético, a grandes rasgos.
fn script(c: char) -> Option<&'static str> {
    if !c.is_alphabetic() {
        return None;
    }
    Some(match c as u32 {
        0x0000..=0x024F | 0x1E00..=0x1EFF => "latina",
        0x0370..=0x03FF | 0x1F00..=0x1FFF => "griega",
        0x0400..=0x052F => "cirílica",
        0x0590..=0x05FF => "hebrea",
        0x0600..=0x06FF => "árabe",
        0x3040..=0x30FF | 0x3400..=0x9FFF | 0xAC00..=0xD7AF => "cjk",
        _ => "otra",
    })
}

/// Tokens raros de un fragmento según la escritura principal del documento.
fn odd_tokens(text: &str, main_script: Option<&str>) -> Vec<String> {
    let mut odd = Vec::new();
    for token in text.split(|c: char| c.is_whitespace()) {
        let token = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '\u{FFFD}');
        if token.is_empty() || token.starts_with("![") || token.contains("](") {
            continue;
        }
        let chars: Vec<char> = token.chars().collect();
        let replacement = chars.contains(&'\u{FFFD}');
        let foreign = main_script.is_some_and(|main| {
            chars
                .iter()
                .filter_map(|c| script(*c))
                .any(|s| s != main && s != "otra")
        });
        // Letras y cifras alternadas dentro de una palabra («wa5», «0f», «l1ne»); los ordinales
        // («19th», «2º») y las referencias de una sola letra («A25») no cuentan.
        let kinds: Vec<bool> = chars
            .iter()
            .filter(|c| c.is_alphanumeric())
            .map(|c| c.is_numeric())
            .collect();
        let switches = kinds.windows(2).filter(|w| w[0] != w[1]).count();
        let letters = kinds.iter().filter(|d| !**d).count();
        let suffix: String = chars
            .iter()
            .skip_while(|c| c.is_numeric())
            .collect::<String>()
            .to_lowercase();
        let ordinal = kinds.first() == Some(&true)
            && switches == 1
            && chars.first() != Some(&'0')
            && ORDINAL_SUFFIXES.contains(&suffix.as_str());
        let code = switches == 1 && letters == 1 && kinds.first() == Some(&false);
        let mixed = switches >= 1 && !ordinal && !code && letters >= 1;
        if replacement || foreign || mixed {
            odd.push(token.to_string());
        }
    }
    odd
}

/// Huella corta de un fragmento: reconoce el fragmento revisado aunque cambie de sitio.
pub fn block_key(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.trim().as_bytes()))[..16].to_string()
}

fn excerpt(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= EXCERPT_CHARS {
        flat
    } else {
        flat.chars().take(EXCERPT_CHARS).collect::<String>() + "…"
    }
}

/// Vocabulario del propio documento: cuántas veces aparece cada palabra y, para las frecuentes, sus
/// variantes con una letra menos (para encontrar palabras casi iguales sin diccionario).
struct Vocabulary {
    counts: HashMap<String, usize>,
    /// Variante sin la letra `i` → (palabra frecuente, `i`).
    deletions: HashMap<String, Vec<(String, usize)>>,
}

fn letters_only(word: &str) -> bool {
    word.chars().all(char::is_alphabetic)
}

fn without(chars: &[char], i: usize) -> String {
    chars
        .iter()
        .enumerate()
        .filter(|(k, _)| *k != i)
        .map(|(_, c)| c)
        .collect()
}

impl Vocabulary {
    fn new<'a>(blocks: impl Iterator<Item = &'a str>) -> Self {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for block in blocks {
            // También las palabras de una letra («a», «y»): sin ellas, «a long» parecería «along».
            for word in block
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty() && letters_only(w))
            {
                *counts.entry(word.to_lowercase()).or_default() += 1;
            }
        }
        let mut deletions: HashMap<String, Vec<(String, usize)>> = HashMap::new();
        for (word, _) in counts
            .iter()
            .filter(|(w, n)| **n >= FREQUENT && w.chars().count() >= 4)
        {
            let chars: Vec<char> = word.chars().collect();
            for i in 0..chars.len() {
                deletions
                    .entry(without(&chars, i))
                    .or_default()
                    .push((word.clone(), i));
            }
        }
        Self { counts, deletions }
    }

    fn count(&self, word: &str) -> usize {
        self.counts.get(word).copied().unwrap_or(0)
    }

    /// Palabra frecuente de la que `word` (única en el documento) parece una mala lectura: le falta
    /// o le sobra una letra interior, o tiene una letra cambiada (interior, o una confusión típica
    /// del OCR). Un cambio al final no cuenta: «horses» no es un error de «horse». En modo estricto
    /// (sin texto de referencia) solo cuentan las confusiones típicas y las letras finas perdidas o
    /// sobrantes, porque un vocabulario rico tiene muchas palabras legítimas casi iguales.
    fn looks_like(&self, word: &str, strict: bool) -> Option<String> {
        let chars: Vec<char> = word.chars().collect();
        if self.count(word) != 1 || chars.len() < 4 || !letters_only(word) {
            return None;
        }
        let last = chars.len() - 1;
        // Le falta una letra interior: `word` es una variante de una frecuente.
        if let Some(found) = self.deletions.get(word).and_then(|v| {
            v.iter().find(|(f, i)| {
                *i > 0
                    && *i < f.chars().count() - 1
                    && (!strict || f.chars().nth(*i).is_some_and(|c| THIN.contains(c)))
            })
        }) {
            return Some(found.0.clone());
        }
        for i in 0..chars.len() {
            let variant = without(&chars, i);
            // Le sobra una letra interior.
            if i > 0
                && i < last
                && (!strict || THIN.contains(chars[i]))
                && self.count(&variant) >= FREQUENT
            {
                return Some(variant);
            }
            // Una letra cambiada en la misma posición.
            if let Some(found) = self.deletions.get(&variant).and_then(|v| {
                v.iter().find(|(f, j)| {
                    *j == i && f.chars().count() == chars.len() && *f != word && {
                        let other = f.chars().nth(i).unwrap_or(' ');
                        let pair = (chars[i].min(other), chars[i].max(other));
                        CONFUSABLE.contains(&pair) || (!strict && i > 0 && i + 1 < last)
                    }
                })
            }) {
                return Some(found.0.clone());
            }
        }
        None
    }

    /// Palabras sospechosas de un fragmento según el vocabulario del documento. Con texto de
    /// referencia solo se miran las palabras que la referencia no contiene.
    fn suspects(&self, block: &str, reference: Option<&Reference>) -> Vec<String> {
        // Palabras separadas por espacios («it's» sigue siendo una sola).
        let tokens: Vec<String> = block
            .split_whitespace()
            .map(|t| {
                t.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
            })
            .filter(|t| !t.is_empty())
            .collect();
        let known = |word: &str| {
            reference.is_some_and(|r| r.has(word, |rest| self.count(rest) >= FREQUENT))
        };
        let strict = reference.is_none();
        let mut found = Vec::new();
        for (i, token) in tokens.iter().enumerate() {
            if known(token) {
                continue;
            }
            if let Some(likely) = self.looks_like(token, strict) {
                found.push(format!("{token} (¿{likely}?)"));
                continue;
            }
            // Trozos que juntos forman una palabra frecuente («qu ite»).
            if let Some(next) = tokens.get(i + 1)
                && letters_only(token)
                && letters_only(next)
                && !known(next)
                && (self.count(token) <= 1 || self.count(next) <= 1)
                && self.count(&format!("{token}{next}")) >= FREQUENT
            {
                found.push(format!("{token} {next}"));
                continue;
            }
            // Dos palabras frecuentes pegadas en una palabra única y larga («ofthe», «riverbank»).
            let chars: Vec<char> = token.chars().collect();
            if self.count(token) == 1 && chars.len() >= 8 && letters_only(token) {
                let glued = (3..chars.len() - 2).any(|k| {
                    let (a, b): (String, String) =
                        (chars[..k].iter().collect(), chars[k..].iter().collect());
                    self.count(&a) >= 5 && self.count(&b) >= 5
                });
                if glued {
                    found.push(token.clone());
                }
            }
        }
        found
    }
}

/// Símbolos del fragmento que el texto de referencia de la página no contiene.
fn foreign_symbols(block: &str, reference: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for c in block.chars() {
        let usual = c.is_alphanumeric()
            || c.is_whitespace()
            || c.is_ascii_punctuation()
            || "«»“”‘’—–…·•°%€£§¶©®™¿¡".contains(c);
        if !usual && !reference.contains(c) && !found.contains(&c.to_string()) {
            found.push(c.to_string());
        }
    }
    found
}

/// ¿Es un fragmento de contenido (no una marca, una imagen ni un bloque vacío)? La app usa la misma
/// regla para numerar los fragmentos de la vista.
pub fn is_content_block(block: &str) -> bool {
    let block = block.trim();
    !block.is_empty() && !block.starts_with("![") && !block.starts_with("<!--")
}

/// Fragmentos de contenido de una página.
fn content_blocks(page_text: &str) -> Vec<&str> {
    page_text
        .split("\n\n")
        .filter(|b| is_content_block(b))
        .map(str::trim)
        .collect()
}

/// Lista de revisión de un resultado con marcas de página. `references` es el texto de cada página
/// según el PDF (o su capa invisible); vacío si no hay referencia.
pub fn review(
    markdown: &str,
    references: &HashMap<u32, String>,
    states: &PageStates,
) -> Vec<ReviewItem> {
    let pages = markdown_pages(markdown);
    // Escritura principal del documento (para reconocer letras de otra).
    let mut scripts: HashMap<&str, usize> = HashMap::new();
    for text in pages.values() {
        for s in text.chars().filter_map(script) {
            *scripts.entry(s).or_default() += 1;
        }
    }
    let main_script = scripts.into_iter().max_by_key(|(_, n)| *n).map(|(s, _)| s);
    let mut numbers: Vec<u32> = pages.keys().copied().collect();
    numbers.sort_unstable();
    let all_blocks: Vec<&str> = numbers
        .iter()
        .flat_map(|p| content_blocks(&pages[p]))
        .collect();
    let vocabulary = Vocabulary::new(all_blocks.iter().copied());
    // Tokens raros que se repiten en muchos fragmentos: sistemáticos, pesan poco.
    let mut odd_spread: HashMap<String, usize> = HashMap::new();
    for block in &all_blocks {
        let unique: HashSet<String> = odd_tokens(block, main_script).into_iter().collect();
        for token in unique {
            *odd_spread.entry(token).or_default() += 1;
        }
    }
    let mut items = Vec::new();
    for page in numbers {
        let text = &pages[&page];
        // Referencia: la página y sus vecinas (un párrafo puede cruzar el salto de página).
        let reference_text: Option<String> = references.get(&page).map(|_| {
            [page.saturating_sub(1), page, page + 1]
                .iter()
                .filter_map(|p| references.get(p).map(String::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        });
        let reference = reference_text.as_deref().map(Reference::new);
        if states.doubtful.contains(&page) && !states.reread.contains(&page) {
            items.push(ReviewItem {
                page,
                block: None,
                key: block_key(text),
                excerpt: excerpt(text),
                risk: 3.0,
                reasons: vec![Reason::DoubtfulPage],
            });
        } else if states.reread.contains(&page) {
            items.push(ReviewItem {
                page,
                block: None,
                key: block_key(text),
                excerpt: excerpt(text),
                risk: 1.0,
                reasons: vec![Reason::VisualReread],
            });
        }
        for (index, block) in content_blocks(text).into_iter().enumerate() {
            let mut reasons = Vec::new();
            let mut risk = 0.0;
            if let Some(reference) = &reference {
                let unsupported: Vec<String> = words(block)
                    .into_iter()
                    .filter(|w| !reference.has(w, |rest| vocabulary.count(rest) >= FREQUENT))
                    .collect();
                if !unsupported.is_empty() {
                    risk += unsupported.len().min(MAX_UNSUPPORTED) as f64;
                    reasons.push(Reason::UnsupportedWords {
                        count: unsupported.len(),
                        examples: unsupported.into_iter().take(MAX_EXAMPLES).collect(),
                    });
                }
            }
            let mut odd = odd_tokens(block, main_script);
            if let Some(reference_text) = &reference_text {
                odd.extend(foreign_symbols(block, reference_text));
            }
            if !odd.is_empty() {
                let weight: f64 = odd
                    .iter()
                    .take(MAX_UNSUPPORTED)
                    .map(|t| {
                        if odd_spread.get(t).copied().unwrap_or(0) >= SYSTEMATIC {
                            0.25
                        } else {
                            2.0
                        }
                    })
                    .sum();
                risk += weight;
                reasons.push(Reason::OddTokens {
                    count: odd.len(),
                    examples: odd.into_iter().take(MAX_EXAMPLES).collect(),
                });
            }
            let suspects = vocabulary.suspects(block, reference.as_ref());
            if !suspects.is_empty() {
                risk += 2.0 * suspects.len().min(MAX_UNSUPPORTED) as f64;
                reasons.push(Reason::SuspectWords {
                    count: suspects.len(),
                    examples: suspects.into_iter().take(MAX_EXAMPLES).collect(),
                });
            }
            if risk > 0.0 {
                items.push(ReviewItem {
                    page,
                    block: Some(index),
                    key: block_key(block),
                    excerpt: excerpt(block),
                    risk,
                    reasons,
                });
            }
        }
    }
    items.sort_by(|a, b| {
        b.risk
            .total_cmp(&a.risk)
            .then(a.page.cmp(&b.page))
            .then(a.block.cmp(&b.block))
    });
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(pages: &[&str]) -> String {
        pages
            .iter()
            .enumerate()
            .map(|(i, text)| format!("<!-- page: {} -->\n\n{text}", i + 1))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    #[test]
    fn words_missing_from_the_pdf_rank_first() {
        let markdown = doc(&[
            "Clean paragraph about rivers.\n\nThe river ran flickermay into the sea.",
            "Another clean paragraph.",
        ]);
        let references = HashMap::from([
            (
                1,
                "Clean paragraph about rivers.\nThe river ran flicker—may into the sea."
                    .to_string(),
            ),
            (2, "Another clean paragraph.".to_string()),
        ]);
        let items = review(&markdown, &references, &PageStates::default());
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].page, items[0].block), (1, Some(1)));
        assert!(matches!(
            &items[0].reasons[0],
            Reason::UnsupportedWords { count: 1, examples } if examples == &["flickermay"]
        ));
    }

    #[test]
    fn hyphenated_words_and_neighbouring_pages_are_not_flagged() {
        // «palabra» está partida en la página 1 y su final cae en la 2.
        let markdown = doc(&["Una pala", "bra entera y una frontera."]);
        let references = HashMap::from([
            (1, "Una pala-\n".to_string()),
            (2, "bra entera y una fron-\ntera.".to_string()),
        ]);
        let markdown = markdown.replace("Una pala", "Una palabra");
        assert!(review(&markdown, &references, &PageStates::default()).is_empty());
    }

    #[test]
    fn odd_tokens_are_found_without_a_reference() {
        let markdown = doc(&[
            "It wa5 the 0f times in 1892, the 19th day, ref. A25, \u{FFFD}ruined and 木 here.",
        ]);
        let items = review(&markdown, &HashMap::new(), &PageStates::default());
        let Reason::OddTokens { examples, .. } = &items[0].reasons[0] else {
            panic!("{items:?}");
        };
        assert_eq!(examples, &["wa5", "0f", "\u{FFFD}ruined", "木"]);
    }

    #[test]
    fn the_document_vocabulary_points_at_misread_words() {
        // «increased» y «quite» son frecuentes; sus malas lecturas aparecen una sola vez.
        let common = "Prices increased quite fast. Rents increased quite a lot. Wages increased quite \
            late.";
        let markdown = doc(&[
            common,
            "Costs inereased, and it was qu ite odd. It's a long way, said the horses.",
        ]);
        let items = review(&markdown, &HashMap::new(), &PageStates::default());
        let suspects: Vec<String> = items
            .iter()
            .flat_map(|i| &i.reasons)
            .filter_map(|r| match r {
                Reason::SuspectWords { examples, .. } => Some(examples.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        // Ni «it's», ni «a long», ni el plural «horses» son sospechosos.
        assert_eq!(suspects, vec!["inereased (¿increased?)", "qu ite"]);
    }

    #[test]
    fn soft_hyphens_and_foreign_symbols_against_the_reference() {
        let markdown = doc(&["A reasonable answer ¤ here."]);
        // PDFium marca el guion de partición con un carácter de control.
        let references = HashMap::from([(1, "A reason\u{0002}\nable answer here.".to_string())]);
        let items = review(&markdown, &references, &PageStates::default());
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].reasons,
            vec![Reason::OddTokens {
                count: 1,
                examples: vec!["¤".into()]
            }]
        );
    }

    #[test]
    fn systematic_odd_tokens_weigh_little() {
        let pages: Vec<String> = (0..5)
            .map(|_| "Line r33 of the proof.".to_string())
            .collect();
        let mut refs: Vec<&str> = pages.iter().map(String::as_str).collect();
        refs.push("A stray wa5 token.");
        let items = review(&doc(&refs), &HashMap::new(), &PageStates::default());
        assert_eq!(items[0].page, 6);
        assert_eq!(items[0].risk, 2.0);
        assert!(items[1..].iter().all(|i| i.risk == 0.25));
    }

    #[test]
    fn page_states_add_page_items_and_order_is_by_risk() {
        let markdown = doc(&["Uno.", "Dos.", "Tres con l1ne rara."]);
        let states = PageStates {
            doubtful: vec![1, 2],
            reread: vec![2],
        };
        let items = review(&markdown, &HashMap::new(), &states);
        let summary: Vec<(u32, Option<usize>, f64)> =
            items.iter().map(|i| (i.page, i.block, i.risk)).collect();
        assert_eq!(
            summary,
            vec![(1, None, 3.0), (3, Some(0), 2.0), (2, None, 1.0)]
        );
    }
}
