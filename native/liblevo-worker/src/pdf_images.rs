//! Imágenes de un PDF digital leídas con PDFium, cada una con su página y su sitio en el texto.
//!
//! Sustituye a la extracción de imágenes de Xberg, que cambia su modo de lectura del texto
//! (pierde rayas, deja más cabeceras y casi no da tablas). Cada imagen se coloca justo antes del
//! primer párrafo que queda por debajo de ella en la página (normalmente su pie de figura); si no se
//! encuentra ese párrafo, al final de la página. Las fotografías guardadas como JPEG se copian byte a
//! byte; el resto se guarda como PNG.

use pdfium_render::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Una imagen guardada y el texto que debe quedar justo después de ella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedImage {
    pub page: u32,
    pub file: PathBuf,
    /// Letras del primer tramo de texto bajo la imagen, si lo hay.
    pub anchor: Option<String>,
}

/// Formularios anidados que se recorren como máximo (un PDF podría anidarlos sin fin).
const MAX_FORM_DEPTH: usize = 4;
/// Letras del tramo siguiente que bastan para encontrarlo en el Markdown.
const ANCHOR_LETTERS: usize = 24;

fn letters(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Caja en coordenadas PDF (el eje vertical crece hacia arriba).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Box {
    top: f64,
    bottom: f64,
    left: f64,
    right: f64,
}

impl Box {
    fn of(rect: PdfRect) -> Self {
        Self {
            top: f64::from(rect.top().value),
            bottom: f64::from(rect.bottom().value),
            left: f64::from(rect.left().value),
            right: f64::from(rect.right().value),
        }
    }

    fn overlaps_horizontally(&self, other: &Box) -> bool {
        self.left.max(other.left) < self.right.min(other.right)
    }
}

struct FoundImage {
    bytes: Vec<u8>,
    extension: &'static str,
    place: Box,
}

/// Imágenes de un grupo de objetos, entrando en los formularios anidados.
fn collect_images<'a>(
    objects: impl Iterator<Item = PdfPageObject<'a>>,
    depth: usize,
    out: &mut Vec<FoundImage>,
) {
    for object in objects {
        if let Some(form) = object.as_x_object_form_object() {
            if depth < MAX_FORM_DEPTH {
                collect_images(form.iter(), depth + 1, out);
            }
            continue;
        }
        let Some(image) = object.as_image_object() else {
            continue;
        };
        let Ok(bounds) = object.bounds() else {
            continue;
        };
        let place = Box::of(bounds.to_rect());
        // Un JPEG sin más filtros ya es un archivo de imagen válido: se copia tal cual.
        let raw = image.get_raw_image_data().unwrap_or_default();
        if raw.starts_with(&[0xFF, 0xD8, 0xFF]) && image::guess_format(&raw).is_ok() {
            out.push(FoundImage {
                bytes: raw,
                extension: "jpeg",
                place,
            });
            continue;
        }
        let Ok(decoded) = image.get_raw_image() else {
            continue;
        };
        let mut png = Vec::new();
        if decoded
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .is_ok()
        {
            out.push(FoundImage {
                bytes: png,
                extension: "png",
                place,
            });
        }
    }
}

/// Primer tramo de texto bajo la imagen y en su misma franja horizontal (el más alto de los que
/// quedan debajo). Así un texto del margen, como una franja vertical, no sirve de ancla.
fn anchor_below(segments: &[(Box, String)], image: &Box) -> Option<String> {
    segments
        .iter()
        .filter(|(place, text)| {
            place.top <= image.bottom + 1.0
                && place.overlaps_horizontally(image)
                && !letters(text).is_empty()
        })
        .max_by(|a, b| a.0.top.total_cmp(&b.0.top))
        .map(|(_, text)| letters(text).chars().take(ANCHOR_LETTERS).collect())
}

/// Guarda en `out_dir` las imágenes de todas las páginas (`p<página>-<n>.<ext>`).
pub fn extract(pdfium: &Pdfium, path: &Path, out_dir: &Path) -> Result<Vec<PlacedImage>, String> {
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("PDF: {e:?}"))?;
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let mut placed = Vec::new();
    for (index, page) in document.pages().iter().enumerate() {
        let number = index as u32 + 1;
        let mut found = Vec::new();
        collect_images(page.objects().iter(), 0, &mut found);
        if found.is_empty() {
            continue;
        }
        let segments: Vec<(Box, String)> = page
            .text()
            .map(|text| {
                text.segments()
                    .iter()
                    .map(|s| (Box::of(s.bounds()), s.text()))
                    .collect()
            })
            .unwrap_or_default();
        for (n, image) in found.into_iter().enumerate() {
            let file = out_dir.join(format!("p{number}-{n}.{}", image.extension));
            std::fs::write(&file, &image.bytes).map_err(|e| e.to_string())?;
            placed.push(PlacedImage {
                page: number,
                file,
                anchor: anchor_below(&segments, &image.place),
            });
        }
    }
    Ok(placed)
}

/// Escala a la que se dibujan las páginas para medir su tinta (basta con ver manchas).
const INK_SCALE: f32 = 0.25;
/// Margen que se ignora en cada borde (el fondo negro o la sombra del escáner).
const INK_MARGIN: f64 = 0.08;
/// Diferencia con el papel a partir de la cual un píxel es tinta (de 0 a 255).
const INK_CONTRAST: u8 = 60;

/// Parte de la zona central de una imagen en gris que es tinta: píxeles bastante más oscuros que
/// el papel (la mediana de la zona).
fn ink_of(gray: &image::GrayImage) -> f64 {
    let (w, h) = gray.dimensions();
    let (x0, y0) = (
        (f64::from(w) * INK_MARGIN) as u32,
        (f64::from(h) * INK_MARGIN * 0.75) as u32,
    );
    let (x1, y1) = (w.saturating_sub(x0), h.saturating_sub(y0));
    let mut values: Vec<u8> = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            values.push(gray.get_pixel(x, y)[0]);
        }
    }
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.clone();
    sorted.sort_unstable();
    let paper = sorted[sorted.len() / 2];
    let dark = values
        .iter()
        .filter(|v| v.saturating_add(INK_CONTRAST) < paper)
        .count();
    dark as f64 / values.len() as f64
}

/// Tinta de cada página (la posición `i` es la página `i + 1`), para la señal de figuras.
pub fn ink_fractions(pdfium: &Pdfium, path: &Path) -> Result<Vec<f64>, String> {
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("PDF: {e:?}"))?;
    let config = PdfRenderConfig::new().scale_page_by_factor(INK_SCALE);
    let mut ink = Vec::new();
    for page in document.pages().iter() {
        let gray = page
            .render_with_config(&config)
            .map_err(|e| format!("{e:?}"))?
            .as_image()
            .map_err(|e| format!("{e:?}"))?
            .to_luma8();
        ink.push(ink_of(&gray));
    }
    Ok(ink)
}

/// Dibuja una página (1 = primera) como PNG, a `scale` veces su tamaño en puntos.
pub fn render_page(
    pdfium: &Pdfium,
    path: &Path,
    page: u32,
    scale: f32,
    out: &Path,
) -> Result<(), String> {
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("PDF: {e:?}"))?;
    let index = page
        .checked_sub(1)
        .and_then(|i| PdfPageIndex::try_from(i).ok())
        .ok_or("página fuera del documento")?;
    let pdf_page = document.pages().get(index).map_err(|e| format!("{e:?}"))?;
    let bitmap = pdf_page
        .render_with_config(&PdfRenderConfig::new().scale_page_by_factor(scale))
        .map_err(|e| format!("{e:?}"))?;
    bitmap
        .as_image()
        .map_err(|e| format!("{e:?}"))?
        .save_with_format(out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())
}

/// Inserta las referencias `![](nombre)` en el Markdown con marcas `<!-- page: N -->`.
pub fn insert_references(markdown: &str, images: &[PlacedImage]) -> String {
    let mut by_page: HashMap<u32, Vec<&PlacedImage>> = HashMap::new();
    for image in images {
        by_page.entry(image.page).or_default().push(image);
    }
    let mut out = Vec::new();
    let mut page_blocks: Vec<String> = Vec::new();
    let mut page: Option<u32> = None;
    let flush = |page: Option<u32>, blocks: &mut Vec<String>, out: &mut Vec<String>| {
        if let Some(images) = page.and_then(|p| by_page.get(&p)) {
            for image in images {
                let name = image
                    .file
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let reference = format!("![]({name})");
                let at = image.anchor.as_ref().and_then(|anchor| {
                    blocks.iter().position(|block| {
                        !block.starts_with("![") && letters(block).contains(anchor)
                    })
                });
                match at {
                    Some(at) => blocks.insert(at, reference),
                    None => blocks.push(reference),
                }
            }
        }
        out.append(blocks);
    };
    for block in markdown.split("\n\n") {
        if let Some(rest) = block.trim().strip_prefix("<!-- page:") {
            flush(page, &mut page_blocks, &mut out);
            page = rest.trim_end_matches("-->").trim().parse().ok();
            out.push(block.to_string());
        } else {
            page_blocks.push(block.to_string());
        }
    }
    flush(page, &mut page_blocks, &mut out);
    out.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(page: u32, name: &str, anchor: Option<&str>) -> PlacedImage {
        PlacedImage {
            page,
            file: PathBuf::from(name),
            anchor: anchor.map(letters),
        }
    }

    #[test]
    fn ink_ignores_paper_tone_and_scanner_borders() {
        // Papel claro con un borde negro de escáner y una mancha central del 10 % de la zona.
        let mut gray = image::GrayImage::from_pixel(100, 100, image::Luma([200]));
        for y in 0..100 {
            for x in 0..4 {
                gray.put_pixel(x, y, image::Luma([0]));
            }
        }
        for y in 40..60 {
            for x in 30..72 {
                gray.put_pixel(x, y, image::Luma([20]));
            }
        }
        let ink = ink_of(&gray);
        assert!((0.09..0.12).contains(&ink), "{ink}");
        // Papel oscuro y uniforme: sin tinta.
        assert_eq!(
            ink_of(&image::GrayImage::from_pixel(50, 50, image::Luma([90]))),
            0.0
        );
    }

    #[test]
    fn image_goes_before_the_text_below_it() {
        let doc = "<!-- page: 1 -->\n\nIntro.\n\nFigure 1: A sketch.\n\nMore text.\n\n<!-- page: 2 -->\n\nNext.";
        let out = insert_references(doc, &[placed(1, "p1-0.png", Some("Figure 1: A"))]);
        assert_eq!(
            out,
            "<!-- page: 1 -->\n\nIntro.\n\n![](p1-0.png)\n\nFigure 1: A sketch.\n\nMore text.\n\n<!-- page: 2 -->\n\nNext."
        );
    }

    #[test]
    fn image_without_anchor_goes_to_the_end_of_its_page() {
        let doc = "<!-- page: 1 -->\n\nUno.\n\n<!-- page: 2 -->\n\nDos.";
        let out = insert_references(
            doc,
            &[
                placed(1, "p1-0.jpeg", None),
                placed(2, "p2-0.png", Some("no está")),
            ],
        );
        assert_eq!(
            out,
            "<!-- page: 1 -->\n\nUno.\n\n![](p1-0.jpeg)\n\n<!-- page: 2 -->\n\nDos.\n\n![](p2-0.png)"
        );
    }

    #[test]
    fn anchor_is_the_nearest_text_below_in_the_same_column() {
        let at = |top: f64, left: f64, right: f64| Box {
            top,
            bottom: top - 10.0,
            left,
            right,
        };
        let segments = vec![
            (at(700.0, 100.0, 500.0), "Título arriba".to_string()),
            (at(400.0, 100.0, 500.0), "Figura 2. Pie".to_string()),
            (at(300.0, 100.0, 500.0), "Texto lejano".to_string()),
            // Franja vertical del margen: más cerca, pero fuera de la columna de la imagen.
            (at(410.0, 10.0, 30.0), "arXiv margen".to_string()),
        ];
        let image = |bottom: f64| Box {
            top: bottom + 200.0,
            bottom,
            left: 150.0,
            right: 450.0,
        };
        assert_eq!(
            anchor_below(&segments, &image(420.0)).as_deref(),
            Some("figura2pie")
        );
        assert_eq!(anchor_below(&segments, &image(100.0)), None);
    }
}
