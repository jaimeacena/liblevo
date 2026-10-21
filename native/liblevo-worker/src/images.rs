//! Selección general de imágenes: conserva las figuras y descarta adornos y fondos.
//!
//! Un PDF guarda todas sus imágenes (`pdf_images`): logotipos repetidos en cada página, iconos
//! diminutos o fondos de página. Se descartan con reglas generales, sin mirar el contenido ni usar
//! nada de un documento concreto:
//!
//! 1. lado menor por debajo de 48 píxeles;
//! 2. la misma imagen (mismo contenido) en tres páginas o más;
//! 3. forma de página completa (vertical, grande) en una página que ya tiene texto;
//! 4. imagen casi uniforme (un solo color), como los rellenos de una figura vectorial;
//! 5. máscara en escala de grises que acompaña a otra imagen del mismo tamaño.

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ImageFacts {
    pub width: u32,
    pub height: u32,
    pub content_hash: String,
    /// Desviación típica de la luminancia por debajo de 4 (de 0 a 255).
    pub uniform: bool,
    /// Un solo canal de gris (las máscaras de transparencia se guardan así).
    pub grayscale: bool,
}

/// Datos de un archivo de imagen, o `None` si no se puede leer.
pub fn facts(path: &Path) -> Option<ImageFacts> {
    let bytes = std::fs::read(path).ok()?;
    let decoded = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let grayscale = matches!(
        decoded.color(),
        image::ColorType::L8 | image::ColorType::L16 | image::ColorType::La8
    );
    let luma = decoded.to_luma8();
    let n = luma.len().max(1) as f64;
    let mean = luma.iter().map(|&v| f64::from(v)).sum::<f64>() / n;
    let variance = luma
        .iter()
        .map(|&v| (f64::from(v) - mean).powi(2))
        .sum::<f64>()
        / n;
    Some(ImageFacts {
        width: decoded.width(),
        height: decoded.height(),
        content_hash: format!("{:x}", Sha256::digest(&bytes)),
        uniform: variance.sqrt() < 4.0,
        grayscale,
    })
}

/// Referencias de imagen `![...](nombre)` con la página en la que aparecen.
fn references(markdown: &str) -> Vec<(u32, String)> {
    let mut page = 0;
    let mut found = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("<!-- page:") {
            page = rest.trim_end_matches("-->").trim().parse().unwrap_or(page);
            continue;
        }
        let mut rest = trimmed;
        while let Some(start) = rest.find("![") {
            let after = &rest[start..];
            let Some(open) = after.find("](") else { break };
            let Some(close) = after[open..].find(')') else {
                break;
            };
            found.push((page, after[open + 2..open + close].trim().to_string()));
            rest = &after[open + close..];
        }
    }
    found
}

fn words_per_page(markdown: &str) -> HashMap<u32, usize> {
    let mut page = 0;
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("<!-- page:") {
            page = rest.trim_end_matches("-->").trim().parse().unwrap_or(page);
            continue;
        }
        if trimmed.starts_with("![") {
            continue;
        }
        *counts.entry(page).or_default() += trimmed
            .split_whitespace()
            .filter(|w| w.chars().any(char::is_alphanumeric))
            .count();
    }
    counts
}

/// En un escaneo, las imágenes incrustadas son las propias páginas (y sus capas de compresión), no
/// figuras: se descartan todas. Las figuras de un escaneo se recortan de la página.
pub fn all_referenced(markdown: &str) -> HashSet<String> {
    references(markdown)
        .into_iter()
        .map(|(_, name)| name)
        .collect()
}

/// Decide qué imágenes se descartan. `facts_of` da los datos de cada nombre referenciado.
pub fn rejected(markdown: &str, facts_of: impl Fn(&str) -> Option<ImageFacts>) -> HashSet<String> {
    let refs = references(markdown);
    let words = words_per_page(markdown);
    let known: HashMap<&str, ImageFacts> = refs
        .iter()
        .filter_map(|(_, name)| facts_of(name).map(|f| (name.as_str(), f)))
        .collect();
    let mut pages_by_hash: HashMap<&str, HashSet<u32>> = HashMap::new();
    for (page, name) in &refs {
        if let Some(f) = known.get(name.as_str()) {
            pages_by_hash
                .entry(&f.content_hash)
                .or_default()
                .insert(*page);
        }
    }
    let mut out = HashSet::new();
    let mut previous: Option<(u32, &ImageFacts)> = None;
    for (page, name) in &refs {
        let Some(f) = known.get(name.as_str()) else {
            continue;
        };
        let mask = f.grayscale
            && previous.is_some_and(|(p, prev)| {
                p == *page && !prev.grayscale && prev.width == f.width && prev.height == f.height
            });
        previous = Some((*page, f));
        let tiny = f.width.min(f.height) < 48;
        let repeated = pages_by_hash[f.content_hash.as_str()].len() >= 3;
        let aspect = f.height as f64 / f.width.max(1) as f64;
        let page_like = (1.2..=1.8).contains(&aspect) && f.width >= 600 && f.height >= 600;
        let text_page = words.get(page).copied().unwrap_or(0) >= 25;
        if tiny || repeated || (page_like && text_page) || f.uniform || mask {
            out.insert(name.clone());
        }
    }
    out
}

/// Quita del Markdown las referencias a las imágenes descartadas.
pub fn drop_references(markdown: &str, rejected: &HashSet<String>) -> String {
    let mut out = markdown.to_string();
    for name in rejected {
        // `![texto alternativo](nombre)`, con cualquier texto alternativo.
        let target = format!("]({name})");
        while let Some(end) = out.find(&target) {
            let Some(start) = out[..end].rfind("![") else {
                break;
            };
            out.replace_range(start..end + target.len(), "");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(width: u32, height: u32, hash: &str) -> ImageFacts {
        ImageFacts {
            width,
            height,
            content_hash: hash.into(),
            uniform: false,
            grayscale: false,
        }
    }

    #[test]
    fn scanned_documents_keep_no_embedded_image() {
        let doc = "<!-- page: 1 -->

Texto

<!-- page: 2 -->

![](image_0.png)

![a](image_1.jpeg)";
        let all = all_referenced(doc);
        assert_eq!(all.len(), 2);
        assert_eq!(drop_references(doc, &all).matches("![").count(), 0);
    }

    #[test]
    fn uniform_fills_and_their_masks_are_rejected() {
        let doc = "<!-- page: 1 -->\n\n![](relleno.jpeg)\n\n![](mascara.png)\n\n![](foto.jpeg)\n\n![](gris.png)";
        let facts_of = |name: &str| {
            let mut f = fact(200, 130, name);
            match name {
                "relleno.jpeg" => f.uniform = true,
                "mascara.png" => f.grayscale = true,
                "gris.png" => {
                    f.grayscale = true;
                    f.width = 640;
                }
                _ => {}
            }
            Some(f)
        };
        let out = rejected(doc, facts_of);
        let expected: HashSet<String> = ["relleno.jpeg", "mascara.png"].map(String::from).into();
        assert_eq!(out, expected);
    }

    fn body(words: usize) -> String {
        (0..words)
            .map(|i| format!("palabra{i}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn logos_icons_and_scan_backgrounds_are_rejected_but_figures_kept() {
        let mut doc = String::new();
        for page in 1..=3 {
            doc.push_str(&format!(
                "<!-- page: {page} -->\n\n![](logo{page}.png)\n\n{}\n\n",
                body(80)
            ));
        }
        doc.push_str(&format!(
            "<!-- page: 4 -->\n\n![](fondo.png)\n\n{}\n\n![](icono.png)\n\n",
            body(80)
        ));
        doc.push_str(
            "<!-- page: 5 -->\n\n![Figura 1](figura.png)\n\nPie de figura.\n\n![](lamina.png)",
        );
        let facts_of = |name: &str| match name {
            n if n.starts_with("logo") => Some(fact(300, 120, "logo")),
            "fondo.png" => Some(fact(2132, 3386, "fondo")),
            "icono.png" => Some(fact(32, 32, "icono")),
            "figura.png" => Some(fact(800, 500, "figura")),
            "lamina.png" => Some(fact(2000, 3000, "lamina")),
            _ => None,
        };
        let out = rejected(&doc, facts_of);
        let expected: HashSet<String> = [
            "logo1.png",
            "logo2.png",
            "logo3.png",
            "fondo.png",
            "icono.png",
        ]
        .map(String::from)
        .into();
        assert_eq!(out, expected);
        let cleaned = drop_references(&doc, &out);
        assert!(!cleaned.contains("logo1.png") && !cleaned.contains("fondo.png"));
        assert!(cleaned.contains("![Figura 1](figura.png)"));
        assert!(cleaned.contains("![](lamina.png)"));
    }
}
