//! Líneas del PDF con su estilo observado (tamaño, negrita, centrado), leídas con PDFium.
//! Alimentan la detección general de títulos (`headings`).

use crate::headings::{OutlineEntry, StyledLine};
use pdfium_render::prelude::*;
use std::collections::HashMap;

/// Un objeto de texto del PDF con su caja y su estilo.
struct Piece {
    center: f64,
    height: f64,
    left: f64,
    right: f64,
    text: String,
    size: f64,
    bold: bool,
}

impl Piece {
    fn letters(&self) -> usize {
        self.text.chars().filter(|c| c.is_alphanumeric()).count()
    }
}

/// Negrita según el carácter de muestra: grosor declarado, refuerzo o nombre de la fuente.
fn is_bold(sample: &PdfPageTextChar) -> bool {
    let name = sample.font_name().to_lowercase();
    sample.font_is_bold_reenforced()
        || ["bold", "black", "heavy"].iter().any(|n| name.contains(n))
        || sample.font_weight().is_some_and(|w| {
            matches!(
                w,
                PdfFontWeight::Weight700Bold | PdfFontWeight::Weight800 | PdfFontWeight::Weight900
            )
        })
}

/// Tramos de texto de la página (segmentos de PDFium: misma línea y mismo estilo). Se consulta
/// la fuente de un solo carácter por tramo: leerla objeto a objeto era muy lento en PDF que
/// guardan cada letra por separado.
fn pieces(page: &PdfPage) -> Vec<Piece> {
    let Ok(text) = page.text() else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for segment in text.segments().iter() {
        let content = segment.text();
        if content.trim().is_empty() {
            continue;
        }
        let Ok(chars) = segment.chars() else {
            continue;
        };
        let Some(sample) = chars
            .iter()
            .find(|c| c.unicode_char().is_some_and(char::is_alphanumeric))
        else {
            continue;
        };
        // El texto invisible (la capa OCR de un escaneo) no informa del estilo impreso.
        if matches!(
            sample.render_mode(),
            Ok(PdfPageTextRenderMode::Invisible | PdfPageTextRenderMode::InvisibleClipping)
        ) {
            continue;
        }
        let bounds = segment.bounds();
        let (top, bottom) = (
            f64::from(bounds.top().value),
            f64::from(bounds.bottom().value),
        );
        result.push(Piece {
            center: (top + bottom) / 2.0,
            height: (top - bottom).abs(),
            left: f64::from(bounds.left().value),
            right: f64::from(bounds.right().value),
            text: content,
            size: f64::from(sample.scaled_font_size().value),
            bold: is_bold(&sample),
        });
    }
    result
}

/// Une los tramos de una línea: con espacio solo si hay un hueco visible entre ellos.
fn join_pieces(line: &[Piece], size: f64) -> String {
    let mut text = String::new();
    let mut previous_right: Option<f64> = None;
    for piece in line {
        let content = piece.text.trim();
        if content.is_empty() {
            continue;
        }
        if let Some(right) = previous_right
            && piece.left - right > size.max(1.0) * 0.25
        {
            text.push(' ');
        }
        text.push_str(content);
        previous_right = Some(piece.right);
    }
    text
}

/// Valor más frecuente de una lista, agrupado en tramos de 3 puntos.
fn most_common_edge(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut counts: HashMap<i64, usize> = HashMap::new();
    for value in values {
        *counts.entry((value / 3.0).round() as i64).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(k, _)| k as f64 * 3.0)
}

/// Pasillo vertical entre dos columnas de texto: (borde izquierdo, borde derecho), o `None` si la
/// página tiene una sola columna. Se busca en el tercio central del área de texto la franja más
/// ancha (al menos 6 puntos) que casi ningún tramo cruza, con al menos una cuarta parte de las
/// letras a cada lado. Los pocos tramos que la cruzan (título, pie de figura a todo el ancho) no
/// la anulan.
fn gutter(pieces: &[Piece]) -> Option<(f64, f64)> {
    let total: usize = pieces.iter().map(Piece::letters).sum();
    if total == 0 {
        return None;
    }
    let left = pieces.iter().map(|p| p.left).fold(f64::MAX, f64::min);
    let right = pieces.iter().map(|p| p.right).fold(f64::MIN, f64::max);
    let width = right - left;
    if width < 100.0 {
        return None;
    }
    let crossing_letters = |x: f64| -> usize {
        pieces
            .iter()
            .filter(|p| p.left < x && p.right > x)
            .map(Piece::letters)
            .sum()
    };
    let (from, to) = (left + width / 3.0, left + width * 2.0 / 3.0);
    let mut best: Option<(f64, f64)> = None;
    let mut run_start: Option<f64> = None;
    let mut x = from;
    while x <= to + 0.5 {
        let open = x <= to && crossing_letters(x) * 20 <= total;
        match (open, run_start) {
            (true, None) => run_start = Some(x),
            (false, Some(start)) => {
                // La franja acaba en el último punto libre.
                if best.is_none_or(|(a, b)| x - 1.0 - start > b - a) {
                    best = Some((start, x - 1.0));
                }
                run_start = None;
            }
            _ => {}
        }
        x += 1.0;
    }
    let (start, end) = best?;
    let side = |f: &dyn Fn(&Piece) -> bool| -> usize {
        pieces.iter().filter(|p| f(p)).map(Piece::letters).sum()
    };
    let on_left = side(&|p| p.right <= start + 1.0);
    let on_right = side(&|p| p.left >= end - 1.0);
    (end - start >= 6.0 && on_left * 4 >= total && on_right * 4 >= total).then_some((start, end))
}

/// Parte de la página a la que pertenece un renglón.
#[derive(Clone, Copy, PartialEq)]
enum Column {
    /// Toda la anchura (página de una columna o renglón que cruza el pasillo).
    Whole,
    Left,
    Right,
}

/// Ordena los renglones para leerlos: entre dos renglones de toda la anchura, primero la columna
/// izquierda entera y después la derecha. Un renglón con tramos a ambos lados del pasillo se
/// parte en dos.
fn by_columns(bands: Vec<Vec<Piece>>, gutter: Option<(f64, f64)>) -> Vec<(Vec<Piece>, Column)> {
    let Some((start, end)) = gutter else {
        return bands.into_iter().map(|b| (b, Column::Whole)).collect();
    };
    let mut ordered = Vec::new();
    let (mut lefts, mut rights) = (Vec::new(), Vec::new());
    for band in bands {
        if band
            .iter()
            .any(|p| p.left < end - 0.5 && p.right > start + 0.5)
        {
            ordered.extend(lefts.drain(..).map(|b| (b, Column::Left)));
            ordered.extend(rights.drain(..).map(|b| (b, Column::Right)));
            ordered.push((band, Column::Whole));
            continue;
        }
        let (left, right): (Vec<Piece>, Vec<Piece>) =
            band.into_iter().partition(|p| p.right <= start + 0.5);
        if !left.is_empty() {
            lefts.push(left);
        }
        if !right.is_empty() {
            rights.push(right);
        }
    }
    ordered.extend(lefts.into_iter().map(|b| (b, Column::Left)));
    ordered.extend(rights.into_iter().map(|b| (b, Column::Right)));
    ordered
}

/// Líneas del PDF: agrupa los objetos de texto por su banda vertical, de arriba abajo y, en las
/// páginas a dos columnas, columna a columna.
pub fn styled_lines(
    pdfium: &Pdfium,
    path: &std::path::Path,
    first_page: u32,
) -> Result<Vec<StyledLine>, String> {
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("PDF: {e:?}"))?;
    let mut result = Vec::new();
    for (index, page) in document.pages().iter().enumerate() {
        let mut all = pieces(&page);
        let gutter = gutter(&all);
        all.sort_by(|a, b| {
            b.center
                .total_cmp(&a.center)
                .then(a.left.total_cmp(&b.left))
        });
        let mut lines: Vec<Vec<Piece>> = Vec::new();
        for piece in all {
            match lines.last_mut() {
                Some(current)
                    if (current[0].center - piece.center).abs()
                        <= current[0].height.max(piece.height) * 0.5 =>
                {
                    current.push(piece)
                }
                _ => lines.push(vec![piece]),
            }
        }
        let lines = by_columns(lines, gutter);
        // Columna de texto de cada parte de la página: bordes izquierdo y derecho más frecuentes.
        let line_left = |l: &[Piece]| l.iter().map(|p| p.left).fold(f64::MAX, f64::min);
        let line_right = |l: &[Piece]| l.iter().map(|p| p.right).fold(f64::MIN, f64::max);
        let edges = |column: Column| {
            let part = || {
                lines
                    .iter()
                    .filter(move |(_, c)| column == Column::Whole || *c == column)
            };
            (
                most_common_edge(part().map(|(l, _)| line_left(l))),
                most_common_edge(part().map(|(l, _)| line_right(l))),
            )
        };
        let bounds: HashMap<u8, (Option<f64>, Option<f64>)> =
            [Column::Whole, Column::Left, Column::Right]
                .into_iter()
                .map(|c| (c as u8, edges(c)))
                .collect();
        let mut previous_center: Option<f64> = None;
        for (mut line, column) in lines {
            let (column_left, column_right) = bounds[&(column as u8)];
            line.sort_by(|a, b| a.left.total_cmp(&b.left));
            // «Falsa negrita»: el mismo texto dibujado dos veces casi encima. Se queda una copia y
            // la línea cuenta como negrita.
            let before = line.len();
            line.dedup_by(|later, earlier| {
                let overlap = earlier.right.min(later.right) - earlier.left.max(later.left);
                later.text.trim() == earlier.text.trim()
                    && overlap > (later.right - later.left).max(0.1) * 0.5
            });
            let faux_bold = line.len() < before;
            let total: usize = line.iter().map(Piece::letters).sum();
            if total == 0 {
                continue;
            }
            let bold_letters: usize = line.iter().filter(|p| p.bold).map(Piece::letters).sum();
            let mut sizes: HashMap<i64, usize> = HashMap::new();
            for piece in &line {
                *sizes.entry((piece.size * 2.0).round() as i64).or_default() += piece.letters();
            }
            let size = sizes
                .into_iter()
                .max_by_key(|(_, n)| *n)
                .map_or(0.0, |(s, _)| s as f64 / 2.0);
            let (left, right) = (line_left(&line), line_right(&line));
            let centered = match (column_left, column_right) {
                (Some(cl), Some(cr)) if cr > cl => {
                    let width = cr - cl;
                    let (indent_left, indent_right) = (left - cl, cr - right);
                    indent_left > width * 0.15
                        && indent_right > width * 0.15
                        && (indent_left - indent_right).abs() < width * 0.08
                }
                _ => false,
            };
            let center = line[0].center;
            result.push(StyledLine {
                page: first_page + index as u32,
                text: join_pieces(&line, size),
                size,
                bold: faux_bold || bold_letters * 5 >= total * 4,
                gap_before: previous_center.map(|p| (p - center).abs()),
                centered,
            });
            previous_center = Some(center);
        }
    }
    Ok(result)
}

/// Texto completo de cada página (1 = primera), con saltos de línea `\n`. A diferencia de
/// `styled_lines`, conserva los tramos sin letras, como una raya que PDFium entrega aparte. Incluye
/// el texto invisible, así que no debe usarse en escaneos.
pub fn page_texts(pdfium: &Pdfium, path: &std::path::Path) -> HashMap<u32, String> {
    let Ok(document) = pdfium.load_pdf_from_file(path, None) else {
        return HashMap::new();
    };
    document
        .pages()
        .iter()
        .enumerate()
        .filter_map(|(index, page)| {
            let text = page.text().ok()?.all().replace("\r\n", "\n");
            (!text.trim().is_empty()).then_some((index as u32 + 1, text))
        })
        .collect()
}

/// Marcadores del PDF (índice navegable del propio documento), con página y nivel.
pub fn outline(pdfium: &Pdfium, path: &std::path::Path) -> Vec<OutlineEntry> {
    let Ok(document) = pdfium.load_pdf_from_file(path, None) else {
        return Vec::new();
    };
    fn collect(bookmark: PdfBookmark, depth: u8, out: &mut Vec<OutlineEntry>) {
        // Límites contra marcadores circulares o enormes en un PDF dañado.
        if depth > 8 || out.len() >= 5_000 {
            return;
        }
        if let (Some(title), Some(page)) = (
            bookmark.title(),
            bookmark
                .destination()
                .and_then(|d| d.page_index().ok())
                .and_then(|i| u32::try_from(i).ok())
                .map(|i| i + 1),
        ) && !title.trim().is_empty()
        {
            out.push(OutlineEntry {
                title: title.trim().to_string(),
                page,
                depth,
            });
        }
        for child in bookmark.iter_direct_children() {
            collect(child, depth + 1, out);
        }
    }
    let mut entries = Vec::new();
    if let Some(first) = document.bookmarks().root() {
        let mut seen_first = false;
        for bookmark in first.iter_siblings() {
            seen_first |= bookmark.title() == first.title();
            collect(bookmark, 0, &mut entries);
        }
        if !seen_first {
            collect(first, 0, &mut entries);
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(center: f64, left: f64, right: f64, text: &str) -> Piece {
        Piece {
            center,
            height: 10.0,
            left,
            right,
            text: text.into(),
            size: 10.0,
            bold: false,
        }
    }

    /// Renglones a la misma altura en dos columnas separadas por un pasillo de 20 puntos.
    fn two_columns() -> Vec<Vec<Piece>> {
        (0..10)
            .map(|i| {
                let y = 700.0 - f64::from(i) * 12.0;
                vec![
                    piece(y, 50.0, 290.0, "texto de la columna izquierda"),
                    piece(y, 310.0, 550.0, "texto de la columna derecha"),
                ]
            })
            .collect()
    }

    #[test]
    fn two_column_pages_are_read_column_by_column() {
        let mut bands = vec![vec![piece(
            720.0,
            120.0,
            480.0,
            "Un título a todo el ancho",
        )]];
        bands.extend(two_columns());
        let flat: Vec<Piece> = two_columns().into_iter().flatten().collect();
        let found = gutter(&flat).expect("pasillo");
        assert!(found.0 >= 290.0 && found.1 <= 310.0);
        let ordered = by_columns(bands, gutter(&flat));
        let columns: Vec<Column> = ordered.iter().map(|(_, c)| *c).collect();
        assert!(columns[0] == Column::Whole);
        assert!(columns[1..11].iter().all(|c| *c == Column::Left));
        assert!(columns[11..].iter().all(|c| *c == Column::Right));
        // Cada renglón partido conserva solo su columna.
        assert!(ordered[1].0.iter().all(|p| p.text.contains("izquierda")));
    }

    #[test]
    fn single_column_pages_and_tables_are_not_split() {
        // Una sola columna: renglones de borde a borde.
        let single: Vec<Piece> = (0..10)
            .map(|i| piece(700.0 - f64::from(i) * 12.0, 50.0, 550.0, "un renglón largo"))
            .collect();
        assert_eq!(gutter(&single), None);
        // Una tabla estrecha dentro de un texto a una columna: su hueco no es un pasillo.
        let mut with_table = single;
        with_table.push(piece(560.0, 50.0, 120.0, "celda"));
        with_table.push(piece(560.0, 400.0, 450.0, "celda"));
        assert_eq!(gutter(&with_table), None);
    }
}
