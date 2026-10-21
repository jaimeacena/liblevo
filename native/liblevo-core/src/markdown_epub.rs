//! Exportación de Markdown a EPUB 3.
//!
//! Divide el libro en capítulos por sus títulos, genera el índice navegable e incluye solo las
//! imágenes referenciadas. El HTML incrustado en el Markdown se convierte en texto: el EPUB queda
//! bien formado y nunca lleva código ejecutable.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Seek, Write};

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Fallo al escribir el EPUB (sin contenido documental).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportError {
    Io,
    Zip,
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ExportError {}

impl From<std::io::Error> for ExportError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

impl From<zip::result::ZipError> for ExportError {
    fn from(_: zip::result::ZipError) -> Self {
        Self::Zip
    }
}

pub struct BookMetadata {
    pub title: String,
    pub author: Option<String>,
    /// Código de idioma BCP 47, por ejemplo `es` o `en`.
    pub language: String,
    /// Identificador estable, por ejemplo la huella del original.
    pub identifier: String,
    /// Fecha de modificación en UTC: `AAAA-MM-DDThh:mm:ssZ`.
    pub modified: String,
    /// Portada (PNG o JPEG): primera página del libro, marcada como `cover-image`.
    pub cover: Option<ImageFile>,
}

pub struct ImageFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Un título dentro de un capítulo, para el índice navegable.
struct TocHeading {
    level: usize,
    id: String,
    text: String,
}

struct TocEntry {
    file: String,
    title: String,
    headings: Vec<TocHeading>,
}

struct Chapter {
    title: String,
    markdown: String,
}

fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    ((1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ')).then_some(hashes)
}

/// Nivel de título por el que se divide en capítulos: el más alto que aparece al menos dos veces.
fn split_level(markdown: &str) -> Option<usize> {
    let mut counts = [0usize; 7];
    let mut fence = false;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if !fence && let Some(level) = heading_level(line) {
            counts[level] += 1;
        }
    }
    (1..=6).find(|&level| counts[level] >= 2)
}

fn chapters(markdown: &str, fallback_title: &str) -> Vec<Chapter> {
    let level = split_level(markdown);
    let mut result: Vec<Chapter> = Vec::new();
    let mut current = Chapter {
        title: fallback_title.to_string(),
        markdown: String::new(),
    };
    let mut fence = false;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        let starts_chapter =
            !fence && level.is_some_and(|split| heading_level(line).is_some_and(|l| l <= split));
        if starts_chapter && !current.markdown.trim().is_empty() {
            result.push(current);
            current = Chapter {
                title: String::new(),
                markdown: String::new(),
            };
        }
        if starts_chapter {
            current.title = line
                .trim_start()
                .trim_start_matches('#')
                .trim()
                .replace('*', "");
        }
        current.markdown.push_str(line);
        current.markdown.push('\n');
    }
    if !current.markdown.trim().is_empty() || result.is_empty() {
        result.push(current);
    }
    result
}

/// Quita los caracteres que XML 1.0 no admite (controles y no-caracteres como U+FFFF), que a
/// veces llegan desde la capa de texto de un PDF.
fn xml_safe(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            matches!(c, '\t' | '\n' | '\r')
                || ('\u{20}'..='\u{D7FF}').contains(&c)
                || ('\u{E000}'..='\u{FFFD}').contains(&c)
                || c >= '\u{10000}'
        })
        .collect()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn media_type(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.ends_with(".png") {
        Some("image/png")
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("image/jpeg")
    } else if lower.ends_with(".gif") {
        Some("image/gif")
    } else if lower.ends_with(".svg") {
        Some("image/svg+xml")
    } else if lower.ends_with(".webp") {
        Some("image/webp")
    } else {
        None
    }
}

/// XHTML de un capítulo, los títulos internos para el índice y las imágenes usadas.
fn render(
    chapter: &Chapter,
    index: usize,
    images: &BTreeSet<String>,
) -> (String, Vec<TocHeading>, BTreeSet<String>) {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_MATH);
    let mut used = BTreeSet::new();
    let mut headings: Vec<TocHeading> = Vec::new();
    let mut counter = 0;
    let mut in_heading: Option<(usize, String)> = None;
    let mut skipping_image = false;
    let mut events: Vec<Event> = Vec::new();
    for event in Parser::new_ext(&chapter.markdown, options) {
        match event {
            // El HTML incrustado se muestra como texto: nada se ejecuta y el XML sigue válido.
            Event::Html(raw) | Event::InlineHtml(raw) => events.push(Event::Text(raw)),
            Event::InlineMath(_) | Event::DisplayMath(_) => events.push(math_event(event)),
            Event::Start(Tag::Heading {
                level,
                classes,
                attrs,
                ..
            }) => {
                counter += 1;
                let id = format!("c{index}-h{counter}");
                in_heading = Some((heading_number(level), id.clone()));
                events.push(Event::Start(Tag::Heading {
                    level,
                    id: Some(CowStr::from(id)),
                    classes,
                    attrs,
                }));
            }
            Event::End(TagEnd::Heading(level)) => {
                if let Some((number, id)) = in_heading.take() {
                    let text = headings_text(&events);
                    headings.push(TocHeading {
                        level: number,
                        id,
                        text,
                    });
                }
                events.push(Event::End(TagEnd::Heading(level)));
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let name = dest_url
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("")
                    .to_string();
                if images.contains(&name) {
                    used.insert(name.clone());
                    events.push(Event::Start(Tag::Image {
                        link_type,
                        dest_url: CowStr::from(format!("images/{name}")),
                        title,
                        id,
                    }));
                } else {
                    // Imagen que no se exporta: queda su texto alternativo, en cursiva.
                    skipping_image = true;
                    events.push(Event::Start(Tag::Emphasis));
                }
            }
            Event::End(TagEnd::Image) if skipping_image => {
                skipping_image = false;
                events.push(Event::End(TagEnd::Emphasis));
            }
            other => events.push(other),
        }
    }
    let mut body = String::new();
    html::push_html(&mut body, events.into_iter());
    (body, headings, used)
}

/// Una fórmula sencilla se escribe con subíndices y superíndices; las demás quedan como texto,
/// con sus `$` (ver `math`).
pub fn math_event(event: Event<'_>) -> Event<'_> {
    let (tex, display) = match &event {
        Event::InlineMath(tex) => (tex.clone(), false),
        Event::DisplayMath(tex) => (tex.clone(), true),
        _ => return event,
    };
    match crate::math::simple_math_html(&tex) {
        Some(html) => Event::InlineHtml(CowStr::from(html)),
        None if display => Event::Text(CowStr::from(format!("$${tex}$$"))),
        None => Event::Text(CowStr::from(format!("${tex}$"))),
    }
}

fn heading_number(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Texto del título que se está cerrando: los eventos de texto desde su inicio.
fn headings_text(events: &[Event]) -> String {
    let start = events
        .iter()
        .rposition(|e| matches!(e, Event::Start(Tag::Heading { .. })))
        .unwrap_or(0);
    events[start..]
        .iter()
        .filter_map(|e| match e {
            Event::Text(t) | Event::Code(t) => Some(t.to_string()),
            _ => None,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn xhtml(title: &str, language: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{lang}\" lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\"/>\n<title>{title}</title>\n<link rel=\"stylesheet\" type=\"text/css\" href=\"styles.css\"/>\n</head>\n<body>\n{body}</body>\n</html>\n",
        lang = escape(language),
        title = escape(title),
    )
}

const STYLES: &str = "body { font-family: serif; line-height: 1.5; margin: 0 5%; }\n\
h1, h2, h3, h4 { font-family: sans-serif; line-height: 1.25; }\n\
img { max-width: 100%; height: auto; }\n\
table { border-collapse: collapse; margin: 1em 0; }\n\
th, td { border: 1px solid #999; padding: 0.2em 0.5em; vertical-align: top; }\n\
.cover { text-align: center; }\n\
.cover img { max-height: 95vh; }\n";

/// Escribe el EPUB en `output`. El llamante decide dónde y cuándo publicarlo.
pub fn write_markdown_epub<W: Write + Seek>(
    markdown: &str,
    metadata: &BookMetadata,
    images: &[ImageFile],
    output: W,
) -> Result<W, ExportError> {
    let available: BTreeSet<String> = images
        .iter()
        .filter(|i| media_type(&i.name).is_some())
        .map(|i| i.name.clone())
        .collect();
    let mut zip = ZipWriter::new(output);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/epub+zip")?;
    zip.start_file("META-INF/container.xml", deflated)?;
    zip.write_all(
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n<rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles>\n</container>\n",
    )?;

    // Portada: una página propia al principio, sin entrada en el índice.
    let cover = metadata
        .cover
        .as_ref()
        .and_then(|image| media_type(&image.name).map(|kind| (image, kind)))
        .filter(|(_, kind)| *kind == "image/png" || *kind == "image/jpeg");
    let cover_name = cover.map(|(_, kind)| {
        let extension = if kind == "image/png" { "png" } else { "jpg" };
        format!("liblevo-portada.{extension}")
    });
    if let (Some((image, _)), Some(name)) = (cover, &cover_name) {
        zip.start_file(format!("OEBPS/images/{name}"), stored)?;
        zip.write_all(&image.bytes)?;
        zip.start_file("OEBPS/cover.xhtml", deflated)?;
        zip.write_all(
            xhtml(
                "Portada",
                &metadata.language,
                &format!(
                    "<section epub:type=\"cover\" class=\"cover\"><img src=\"images/{name}\" alt=\"Portada\"/></section>\n"
                ),
            )
            .as_bytes(),
        )?;
    }

    let mut used_images = BTreeSet::new();
    let mut toc: Vec<TocEntry> = Vec::new();
    let markdown = xml_safe(markdown);
    let parts = chapters(&markdown, &metadata.title);
    for (index, chapter) in parts.iter().enumerate() {
        let (body, headings, used) = render(chapter, index + 1, &available);
        used_images.extend(used);
        let file = format!("chapter-{:03}.xhtml", index + 1);
        let title = if chapter.title.is_empty() {
            metadata.title.clone()
        } else {
            chapter.title.clone()
        };
        zip.start_file(format!("OEBPS/{file}"), deflated)?;
        zip.write_all(xhtml(&title, &metadata.language, &body).as_bytes())?;
        toc.push(TocEntry {
            file,
            title,
            headings,
        });
    }

    let mut nav = String::from("<nav epub:type=\"toc\" id=\"toc\"><h1>Índice</h1>\n<ol>\n");
    for TocEntry {
        file,
        title,
        headings,
    } in &toc
    {
        nav.push_str(&format!("<li><a href=\"{file}\">{}</a>", escape(title)));
        let base = headings.iter().map(|h| h.level).min().unwrap_or(1);
        let inner: Vec<&TocHeading> = headings
            .iter()
            .skip(1)
            .filter(|h| h.level == base + 1 && !h.text.is_empty())
            .collect();
        if !inner.is_empty() {
            nav.push_str("\n<ol>\n");
            for TocHeading { id, text, .. } in inner {
                nav.push_str(&format!(
                    "<li><a href=\"{file}#{id}\">{}</a></li>\n",
                    escape(text)
                ));
            }
            nav.push_str("</ol>\n");
        }
        nav.push_str("</li>\n");
    }
    nav.push_str("</ol>\n</nav>\n");
    zip.start_file("OEBPS/nav.xhtml", deflated)?;
    zip.write_all(xhtml("Índice", &metadata.language, &nav).as_bytes())?;
    zip.start_file("OEBPS/styles.css", deflated)?;
    zip.write_all(STYLES.as_bytes())?;

    let mut manifest = String::from(
        "<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n<item id=\"css\" href=\"styles.css\" media-type=\"text/css\"/>\n",
    );
    let mut spine = String::new();
    let mut cover_meta = String::new();
    if let (Some((_, kind)), Some(name)) = (cover, &cover_name) {
        manifest.push_str(&format!(
            "<item id=\"cover-image\" href=\"images/{name}\" media-type=\"{kind}\" properties=\"cover-image\"/>\n<item id=\"cover\" href=\"cover.xhtml\" media-type=\"application/xhtml+xml\"/>\n"
        ));
        spine.push_str("<itemref idref=\"cover\"/>\n");
        cover_meta.push_str("<meta name=\"cover\" content=\"cover-image\"/>\n");
    }
    for (index, TocEntry { file, .. }) in toc.iter().enumerate() {
        manifest.push_str(&format!(
            "<item id=\"c{n}\" href=\"{file}\" media-type=\"application/xhtml+xml\"/>\n",
            n = index + 1
        ));
        spine.push_str(&format!("<itemref idref=\"c{}\"/>\n", index + 1));
    }
    let by_name: BTreeMap<&str, &ImageFile> = images.iter().map(|i| (i.name.as_str(), i)).collect();
    for (index, name) in used_images.iter().enumerate() {
        let image = by_name[name.as_str()];
        zip.start_file(format!("OEBPS/images/{name}"), stored)?;
        zip.write_all(&image.bytes)?;
        manifest.push_str(&format!(
            "<item id=\"img{index}\" href=\"images/{}\" media-type=\"{}\"/>\n",
            escape(name),
            media_type(name).unwrap_or("application/octet-stream")
        ));
    }
    let creator = metadata
        .author
        .as_deref()
        .map(|a| format!("<dc:creator>{}</dc:creator>\n", escape(a)))
        .unwrap_or_default();
    let opf = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"bookid\" xml:lang=\"{lang}\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n<dc:identifier id=\"bookid\">{id}</dc:identifier>\n<dc:title>{title}</dc:title>\n<dc:language>{lang}</dc:language>\n{creator}{cover_meta}<meta property=\"dcterms:modified\">{modified}</meta>\n</metadata>\n<manifest>\n{manifest}</manifest>\n<spine>\n{spine}</spine>\n</package>\n",
        lang = escape(&metadata.language),
        id = escape(&metadata.identifier),
        title = escape(&metadata.title),
        modified = escape(&metadata.modified),
    );
    zip.start_file("OEBPS/content.opf", deflated)?;
    zip.write_all(opf.as_bytes())?;
    Ok(zip.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    fn metadata() -> BookMetadata {
        BookMetadata {
            title: "Libro de prueba".into(),
            author: Some("Autora & Cía".into()),
            language: "es".into(),
            identifier: "urn:liblevo:prueba".into(),
            modified: "2026-10-19T00:00:00Z".into(),
            cover: None,
        }
    }

    fn build(markdown: &str, images: &[ImageFile]) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let bytes = write_markdown_epub(markdown, &metadata(), images, Cursor::new(Vec::new()))
            .unwrap()
            .into_inner();
        zip::ZipArchive::new(Cursor::new(bytes)).unwrap()
    }

    fn read(archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>, name: &str) -> String {
        let mut text = String::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        text
    }

    fn well_formed(xml: &str) {
        let mut reader = quick_xml::Reader::from_str(xml);
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(error) => panic!("XML mal formado: {error}"),
            }
        }
    }

    #[test]
    fn chapters_toc_and_package_are_valid() {
        let markdown = "Prólogo sin título.\n\n# Uno\n\nTexto *uno* & más.\n\n## Sección\n\nAlgo.\n\n# Dos\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let mut archive = build(markdown, &[]);
        assert_eq!(archive.by_index(0).unwrap().name(), "mimetype");
        assert_eq!(
            archive.by_index(0).unwrap().compression(),
            CompressionMethod::Stored
        );
        let names: Vec<String> = archive.file_names().map(String::from).collect();
        for name in [
            "OEBPS/chapter-001.xhtml",
            "OEBPS/chapter-002.xhtml",
            "OEBPS/chapter-003.xhtml",
        ] {
            assert!(names.contains(&name.to_string()), "falta {name}");
        }
        for name in names
            .iter()
            .filter(|n| n.ends_with(".xhtml") || n.ends_with(".opf"))
        {
            well_formed(&read(&mut archive, name));
        }
        let nav = read(&mut archive, "OEBPS/nav.xhtml");
        assert!(
            nav.contains(">Uno</a>") && nav.contains(">Dos</a>") && nav.contains(">Sección</a>")
        );
        let opf = read(&mut archive, "OEBPS/content.opf");
        assert!(opf.contains("<dc:creator>Autora &amp; Cía</dc:creator>"));
        assert!(read(&mut archive, "OEBPS/chapter-003.xhtml").contains("<table>"));
    }

    #[test]
    fn simple_formulas_are_written_and_dollars_in_prose_survive() {
        let mut archive = build(
            "# Uno\n\nGummite $U_{2}O_{3}$ y $\\frac{a}{b}$. Cuesta $8 a la semana y $30 al mes.",
            &[],
        );
        let mut chapter = String::new();
        archive
            .by_name("OEBPS/chapter-001.xhtml")
            .unwrap()
            .read_to_string(&mut chapter)
            .unwrap();
        assert!(chapter.contains("U<sub>2</sub>O<sub>3</sub>"), "{chapter}");
        assert!(chapter.contains("$\\frac{a}{b}$"), "{chapter}");
        assert!(
            chapter.contains("Cuesta $8 a la semana y $30 al mes."),
            "{chapter}"
        );
    }

    #[test]
    fn the_cover_opens_the_book_without_entering_the_index() {
        let mut with_cover = metadata();
        with_cover.cover = Some(ImageFile {
            name: "pagina-1.png".into(),
            bytes: b"png".to_vec(),
        });
        let bytes =
            write_markdown_epub("# Uno\n\nTexto.", &with_cover, &[], Cursor::new(Vec::new()))
                .unwrap()
                .into_inner();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut opf = String::new();
        archive
            .by_name("OEBPS/content.opf")
            .unwrap()
            .read_to_string(&mut opf)
            .unwrap();
        assert!(opf.contains("properties=\"cover-image\""));
        assert!(opf.contains("<spine>\n<itemref idref=\"cover\"/>\n<itemref idref=\"c1\"/>"));
        let mut nav = String::new();
        archive
            .by_name("OEBPS/nav.xhtml")
            .unwrap()
            .read_to_string(&mut nav)
            .unwrap();
        assert!(!nav.contains("cover.xhtml"));
        assert!(archive.by_name("OEBPS/images/liblevo-portada.png").is_ok());
    }

    #[test]
    fn embedded_html_becomes_text_and_only_referenced_images_are_included() {
        let markdown = "# Uno\n\n<script>alert(1)</script>\n\n![Figura](fig.png)\n\n![Perdida](no-esta.png)\n\n# Dos\n\nFin.";
        let images = [
            ImageFile {
                name: "fig.png".into(),
                bytes: vec![137, 80, 78, 71],
            },
            ImageFile {
                name: "sobra.png".into(),
                bytes: vec![1, 2, 3],
            },
        ];
        let mut archive = build(markdown, &images);
        let chapter = read(&mut archive, "OEBPS/chapter-001.xhtml");
        well_formed(&chapter);
        assert!(!chapter.contains("<script"));
        assert!(chapter.contains("&lt;script&gt;"));
        assert!(chapter.contains("src=\"images/fig.png\""));
        assert!(chapter.contains("<em>Perdida</em>"));
        let names: Vec<String> = archive.file_names().map(String::from).collect();
        assert!(names.contains(&"OEBPS/images/fig.png".to_string()));
        assert!(!names.iter().any(|n| n.contains("sobra")));
    }

    #[test]
    fn characters_invalid_in_xml_are_removed() {
        let mut archive = build(
            "# Uno\n\nTexto\u{FFFF} con\u{0001} restos.\n\n# Dos\n\nFin.",
            &[],
        );
        let chapter = read(&mut archive, "OEBPS/chapter-001.xhtml");
        well_formed(&chapter);
        assert!(chapter.contains("Texto con restos."));
    }

    #[test]
    fn a_book_without_repeated_headings_is_one_chapter() {
        let mut archive = build("# Único título\n\nTexto.", &[]);
        let names: Vec<String> = archive.file_names().map(String::from).collect();
        assert!(names.contains(&"OEBPS/chapter-001.xhtml".to_string()));
        assert!(!names.contains(&"OEBPS/chapter-002.xhtml".to_string()));
        well_formed(&read(&mut archive, "OEBPS/nav.xhtml"));
    }
}
