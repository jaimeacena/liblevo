//! EPUB ajeno con el paquete como autoridad: se abre, se edita solo donde la correspondencia
//! es demostrable y se guarda sin tocar lo demás.
//!
//! - Se leen `META-INF/container.xml`, el OPF y el orden de lectura (`spine`).
//! - Fragmentos: los bloques de texto de cada capítulo (párrafos, títulos, elementos de lista,
//!   celdas…). Solo es editable un bloque que contiene únicamente texto; uno con formato interno
//!   («<em>», enlaces, saltos) se muestra pero no se toca: se remite a Sigil o Calibre.
//! - Al guardar, cada entrada sin cambios se copia byte a byte (también su compresión) y solo se
//!   reescriben los capítulos editados. Guardar sin cambios conserva todo el contenido.

use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Elementos que forman un bloque de texto.
const BLOCKS: [&str; 14] = [
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "dt",
    "dd",
    "td",
    "th",
    "figcaption",
    "caption",
];

#[derive(Debug, PartialEq, Eq)]
pub enum PackageError {
    /// No es un ZIP o le falta el contenedor, el OPF o el orden de lectura.
    NotEpub,
    Io,
    /// El bloque no existe o tiene formato interno.
    NotEditable,
}

/// Un bloque de texto de un capítulo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlock {
    pub chapter: usize,
    pub index: usize,
    pub tag: String,
    pub text: String,
    pub editable: bool,
    /// Posición (en bytes) del contenido del bloque dentro del XHTML del capítulo.
    range: (usize, usize),
}

pub struct EpubPackage<R: Read + Seek> {
    archive: ZipArchive<R>,
    /// Rutas dentro del ZIP de los capítulos, en orden de lectura.
    pub chapters: Vec<String>,
    /// Ruta del OPF dentro del ZIP.
    opf_path: String,
    /// XHTML editado de cada capítulo cambiado (y el OPF, si cambia el idioma).
    edited: HashMap<String, String>,
}

/// Sustituye el valor de un atributo (`name="..."`) dentro de una etiqueta, si lo tiene.
fn replace_attribute(tag: &str, name: &str, value: &str) -> String {
    let mut out = String::with_capacity(tag.len());
    let mut rest = tag;
    let pattern = format!(" {name}=");
    while let Some(at) = rest.find(&pattern) {
        let quote_at = at + pattern.len();
        let Some(quote) = rest[quote_at..]
            .chars()
            .next()
            .filter(|c| *c == '"' || *c == '\'')
        else {
            out.push_str(&rest[..quote_at]);
            rest = &rest[quote_at..];
            continue;
        };
        let Some(end) = rest[quote_at + 1..].find(quote) else {
            break;
        };
        out.push_str(&rest[..quote_at + 1]);
        out.push_str(value);
        rest = &rest[quote_at + 1 + end..];
    }
    out.push_str(rest);
    out
}

/// Decodifica los `%XX` de una ruta del manifiesto.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(value) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(value);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Une una ruta relativa al directorio del OPF y resuelve `..` y `.`.
fn join(base: &str, relative: &str) -> String {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn attribute(element: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

fn read_entry<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str) -> Option<String> {
    let mut file = archive.by_name(name).ok()?;
    let mut text = String::new();
    file.read_to_string(&mut text).ok()?;
    Some(text)
}

impl<R: Read + Seek> EpubPackage<R> {
    pub fn open(source: R) -> Result<Self, PackageError> {
        let mut archive = ZipArchive::new(source).map_err(|_| PackageError::NotEpub)?;
        let container =
            read_entry(&mut archive, "META-INF/container.xml").ok_or(PackageError::NotEpub)?;
        let mut opf_path = None;
        let mut reader = Reader::from_str(&container);
        loop {
            match reader.read_event() {
                Ok(Event::Start(e) | Event::Empty(e)) if e.local_name().as_ref() == b"rootfile" => {
                    opf_path = attribute(&e, b"full-path");
                    break;
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        let opf_path = opf_path.ok_or(PackageError::NotEpub)?;
        let opf = read_entry(&mut archive, &opf_path).ok_or(PackageError::NotEpub)?;
        let base = opf_path
            .rsplit_once('/')
            .map_or("", |(dir, _)| dir)
            .to_string();
        let mut manifest: HashMap<String, String> = HashMap::new();
        let mut spine: Vec<String> = Vec::new();
        let mut reader = Reader::from_str(&opf);
        loop {
            match reader.read_event() {
                Ok(Event::Start(e) | Event::Empty(e)) => match e.local_name().as_ref() {
                    b"item" => {
                        if let (Some(id), Some(href)) =
                            (attribute(&e, b"id"), attribute(&e, b"href"))
                        {
                            manifest.insert(id, join(&base, &percent_decode(&href)));
                        }
                    }
                    b"itemref" => {
                        if let Some(idref) = attribute(&e, b"idref") {
                            spine.push(idref);
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        let chapters: Vec<String> = spine
            .iter()
            .filter_map(|id| manifest.get(id).cloned())
            .filter(|path| archive.index_for_name(path).is_some())
            .collect();
        if chapters.is_empty() {
            return Err(PackageError::NotEpub);
        }
        Ok(Self {
            archive,
            chapters,
            opf_path,
            edited: HashMap::new(),
        })
    }

    /// XHTML actual de un capítulo (con las ediciones aplicadas).
    fn chapter_text(&mut self, chapter: usize) -> Option<String> {
        let path = self.chapters.get(chapter)?.clone();
        if let Some(edited) = self.edited.get(&path) {
            return Some(edited.clone());
        }
        read_entry(&mut self.archive, &path)
    }

    /// Bloques de texto de un capítulo.
    pub fn blocks(&mut self, chapter: usize) -> Vec<TextBlock> {
        self.chapter_text(chapter)
            .map(|xhtml| blocks_of(&xhtml, chapter))
            .unwrap_or_default()
    }

    /// Cambia el texto de un bloque editable (solo texto, sin formato interno).
    pub fn set_text(
        &mut self,
        chapter: usize,
        index: usize,
        text: &str,
    ) -> Result<(), PackageError> {
        let xhtml = self
            .chapter_text(chapter)
            .ok_or(PackageError::NotEditable)?;
        let block = blocks_of(&xhtml, chapter)
            .into_iter()
            .find(|b| b.index == index && b.editable)
            .ok_or(PackageError::NotEditable)?;
        let escaped = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let (start, end) = block.range;
        let updated = format!("{}{}{}", &xhtml[..start], escaped, &xhtml[end..]);
        self.edited.insert(self.chapters[chapter].clone(), updated);
        Ok(())
    }

    /// Declara el idioma del libro: el primer `dc:language` del OPF y los atributos `lang` y
    /// `xml:lang` de la etiqueta `<html>` de cada capítulo que los tenga. No toca nada más.
    pub fn set_language(&mut self, code: &str) -> Result<(), PackageError> {
        let opf_path = self.opf_path.clone();
        let opf = match self.edited.get(&opf_path) {
            Some(edited) => edited.clone(),
            None => read_entry(&mut self.archive, &opf_path).ok_or(PackageError::Io)?,
        };
        if let Some(start) = opf.find("<dc:language")
            && let Some(open_end) = opf[start..].find('>').map(|i| start + i + 1)
            && let Some(close) = opf[open_end..].find("</dc:language>").map(|i| open_end + i)
        {
            let updated = format!("{}{code}{}", &opf[..open_end], &opf[close..]);
            self.edited.insert(opf_path, updated);
        }
        for chapter in 0..self.chapters.len() {
            let Some(xhtml) = self.chapter_text(chapter) else {
                continue;
            };
            let Some(start) = xhtml.find("<html") else {
                continue;
            };
            let Some(end) = xhtml[start..].find('>').map(|i| start + i) else {
                continue;
            };
            let tag = &xhtml[start..end];
            let updated_tag =
                replace_attribute(&replace_attribute(tag, "lang", code), "xml:lang", code);
            if updated_tag != tag {
                let updated = format!("{}{updated_tag}{}", &xhtml[..start], &xhtml[end..]);
                self.edited.insert(self.chapters[chapter].clone(), updated);
            }
        }
        Ok(())
    }

    /// ¿Hay ediciones sin guardar?
    pub fn is_edited(&self) -> bool {
        !self.edited.is_empty()
    }

    /// Escribe el EPUB: las entradas sin cambios se copian tal cual; los capítulos editados se
    /// comprimen de nuevo. El orden de las entradas se conserva (`mimetype` sigue primero).
    pub fn save<W: Write + Seek>(&mut self, output: W) -> Result<W, PackageError> {
        let mut writer = ZipWriter::new(output);
        for i in 0..self.archive.len() {
            let name = self
                .archive
                .by_index_raw(i)
                .map_err(|_| PackageError::Io)?
                .name()
                .to_string();
            match self.edited.get(&name) {
                Some(xhtml) => {
                    let options = SimpleFileOptions::default()
                        .compression_method(CompressionMethod::Deflated);
                    writer
                        .start_file(name, options)
                        .map_err(|_| PackageError::Io)?;
                    writer
                        .write_all(xhtml.as_bytes())
                        .map_err(|_| PackageError::Io)?;
                }
                None => {
                    let file = self.archive.by_index_raw(i).map_err(|_| PackageError::Io)?;
                    writer.raw_copy_file(file).map_err(|_| PackageError::Io)?;
                }
            }
        }
        writer.finish().map_err(|_| PackageError::Io)
    }
}

struct Frame {
    name: String,
    block: bool,
    content_start: usize,
    text: String,
    has_element: bool,
    has_block: bool,
}

/// Bloques de texto de un XHTML, con su posición para poder editarlos.
fn blocks_of(xhtml: &str, chapter: usize) -> Vec<TextBlock> {
    let mut reader = Reader::from_str(xhtml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<Frame> = Vec::new();
    let mut out = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        let event = match reader.read_event() {
            Ok(event) => event,
            Err(_) => break,
        };
        let after = reader.buffer_position() as usize;
        match event {
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_lowercase();
                let block = BLOCKS.contains(&name.as_str());
                for frame in stack.iter_mut() {
                    frame.has_element = true;
                    frame.has_block |= block;
                }
                stack.push(Frame {
                    name,
                    block,
                    content_start: after,
                    text: String::new(),
                    has_element: false,
                    has_block: false,
                });
            }
            Event::Empty(_) => {
                for frame in stack.iter_mut() {
                    frame.has_element = true;
                }
            }
            Event::Text(t) => {
                let text = t.unescape().map(|c| c.into_owned()).unwrap_or_default();
                for frame in stack.iter_mut() {
                    frame.text.push_str(&text);
                }
            }
            Event::CData(c) => {
                let text = String::from_utf8_lossy(&c).into_owned();
                for frame in stack.iter_mut() {
                    frame.text.push_str(&text);
                    frame.has_element = true;
                }
            }
            Event::End(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_lowercase();
                // Cierra hasta el elemento correspondiente (XHTML bien formado: es el último).
                while let Some(frame) = stack.pop() {
                    let matched = frame.name == name;
                    if frame.block && !frame.has_block {
                        let text = frame.text.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !text.is_empty() {
                            out.push(TextBlock {
                                chapter,
                                index: out.len(),
                                tag: frame.name.clone(),
                                text,
                                editable: !frame.has_element,
                                range: (frame.content_start, before),
                            });
                        }
                    }
                    if matched {
                        break;
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample() -> Vec<u8> {
        sample_with("", "")
    }

    fn sample_with(metadata: &str, html_attributes: &str) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();
        zip.start_file("META-INF/container.xml", deflated).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
        )
        .unwrap();
        zip.start_file("OEBPS/content.opf", deflated).unwrap();
        zip.write_all(
            format!(r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0">{metadata}<manifest><item id="c1" href="text/cap%201.xhtml" media-type="application/xhtml+xml"/><item id="css" href="style.css" media-type="text/css"/></manifest><spine><itemref idref="c1"/></spine></package>"#).as_bytes(),
        )
        .unwrap();
        zip.start_file("OEBPS/text/cap 1.xhtml", deflated).unwrap();
        zip.write_all(
            format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"{html_attributes}><body>\n<h1>Capítulo</h1>\n<p>Un párrafo &amp; texto.</p>\n<p>Con <em>énfasis</em>.</p>\n<ul><li><p>Dentro de lista</p></li></ul>\n</body></html>")
                .as_bytes(),
        )
        .unwrap();
        zip.start_file("OEBPS/style.css", deflated).unwrap();
        zip.write_all(b"p { margin: 0 }").unwrap();
        zip.finish().unwrap().into_inner()
    }

    fn entries(bytes: &[u8]) -> Vec<(String, CompressionMethod, Vec<u8>)> {
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        (0..archive.len())
            .map(|i| {
                let mut file = archive.by_index(i).unwrap();
                let mut data = Vec::new();
                file.read_to_end(&mut data).unwrap();
                (file.name().to_string(), file.compression(), data)
            })
            .collect()
    }

    #[test]
    fn saving_without_changes_keeps_every_entry() {
        let original = sample();
        let mut package = EpubPackage::open(Cursor::new(original.clone())).unwrap();
        assert_eq!(package.chapters, vec!["OEBPS/text/cap 1.xhtml"]);
        assert!(!package.is_edited());
        let saved = package.save(Cursor::new(Vec::new())).unwrap().into_inner();
        assert_eq!(entries(&saved), entries(&original));
    }

    #[test]
    fn the_language_is_declared_without_touching_anything_else() {
        let bytes = sample_with(
            r#"<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:language>en</dc:language></metadata>"#,
            r#" xml:lang="en" lang='en'"#,
        );
        let mut package = EpubPackage::open(Cursor::new(bytes.clone())).unwrap();
        package.set_language("es").unwrap();
        let saved = package.save(Cursor::new(Vec::new())).unwrap().into_inner();
        let (before, after) = (entries(&bytes), entries(&saved));
        assert_eq!(before.len(), after.len());
        for ((name, _, a), (_, _, b)) in before.iter().zip(&after) {
            let (a, b) = (String::from_utf8_lossy(a), String::from_utf8_lossy(b));
            match name.as_str() {
                "OEBPS/content.opf" => assert_eq!(b, a.replace(">en<", ">es<")),
                "OEBPS/text/cap 1.xhtml" => assert_eq!(
                    b,
                    a.replace(r#"xml:lang="en" lang='en'"#, r#"xml:lang="es" lang='es'"#)
                ),
                _ => assert_eq!(a, b),
            }
        }
        // Sin idioma declarado, nada cambia.
        let mut plain = EpubPackage::open(Cursor::new(sample())).unwrap();
        plain.set_language("es").unwrap();
        assert!(!plain.is_edited());
    }

    #[test]
    fn only_plain_text_blocks_are_editable_and_only_their_text_changes() {
        let original = sample();
        let mut package = EpubPackage::open(Cursor::new(original.clone())).unwrap();
        let blocks = package.blocks(0);
        let summary: Vec<(&str, &str, bool)> = blocks
            .iter()
            .map(|b| (b.tag.as_str(), b.text.as_str(), b.editable))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("h1", "Capítulo", true),
                ("p", "Un párrafo & texto.", true),
                ("p", "Con énfasis.", false),
                ("p", "Dentro de lista", true),
            ]
        );
        assert_eq!(package.set_text(0, 2, "x"), Err(PackageError::NotEditable));
        package.set_text(0, 1, "Corregido <y> & bien").unwrap();
        assert!(package.is_edited());
        assert_eq!(package.blocks(0)[1].text, "Corregido <y> & bien");
        let saved = package.save(Cursor::new(Vec::new())).unwrap().into_inner();
        let (before, after) = (entries(&original), entries(&saved));
        for ((name, method, data), (name2, method2, data2)) in before.iter().zip(&after) {
            assert_eq!((name, method), (name2, method2));
            if name.ends_with(".xhtml") {
                let text = String::from_utf8(data2.clone()).unwrap();
                assert!(text.contains("<p>Corregido &lt;y&gt; &amp; bien</p>"));
                assert_eq!(
                    text.replace("<p>Corregido &lt;y&gt; &amp; bien</p>", ""),
                    String::from_utf8(data.clone())
                        .unwrap()
                        .replace("<p>Un párrafo &amp; texto.</p>", "")
                );
            } else {
                assert_eq!(data, data2, "{name}");
            }
        }
    }

    #[test]
    fn a_zip_without_container_is_not_an_epub() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("a.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"x").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(matches!(
            EpubPackage::open(Cursor::new(bytes)),
            Err(PackageError::NotEpub)
        ));
    }
}
