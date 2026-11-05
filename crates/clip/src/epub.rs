//! The clip as an EPUB 3 book: `mimetype` first and stored, the container, the package, the navigation document, one
//! chapter, a style sheet and the pictures. Every entry has the same fixed date and there is nothing else that varies,
//! so the same input gives the same bytes.

use std::io::Write;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::error::ClipError;
use crate::images::Picture;
use crate::xhtml::escape;

pub struct Book<'a> {
    pub title: &'a str,
    pub byline: Option<&'a str>,
    pub language: &'a str,
    pub source: &'a str,
    pub date: Option<&'a str>,
    /// `YYYY-MM-DDThh:mm:ssZ`, the time of the clip.
    pub modified: &'a str,
    /// `YYYY-MM-DD`
    pub clipped_on: &'a str,
    /// `urn:uuid:…`
    pub identifier: &'a str,
    /// The article, already XHTML.
    pub body: &'a str,
    pub pictures: &'a [Picture],
}

const STYLE: &str = "img { max-width: 100%; height: auto; }\npre { white-space: pre-wrap; overflow-wrap: anywhere; }\ntable { max-width: 100%; }\n.byline, .source { font-size: 0.9em; opacity: 0.75; margin: 0.2em 0; }\n";

fn chapter(b: &Book) -> String {
    let byline = b
        .byline
        .map(|a| format!("<p class=\"byline\">{}</p>", escape(a)))
        .unwrap_or_default();
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"{lang}\" xml:lang=\"{lang}\"><head><meta charset=\"utf-8\"/><title>{title}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/></head><body><h1>{title}</h1>{byline}<p class=\"source\">Clipped from <a href=\"{src}\">{src}</a> on {on}</p>{body}</body></html>",
        lang = escape(b.language),
        title = escape(b.title),
        src = escape(b.source),
        on = escape(b.clipped_on),
        body = b.body,
    )
}

fn package(b: &Book) -> String {
    let creator = b
        .byline
        .map(|a| format!("<dc:creator>{}</dc:creator>", escape(a)))
        .unwrap_or_default();
    let date = b
        .date
        .map(|d| format!("<dc:date>{}</dc:date>", escape(d)))
        .unwrap_or_default();
    let pictures: String = b
        .pictures
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                "<item id=\"img{i}\" href=\"{}\" media-type=\"{}\"/>",
                p.name, p.media_type
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"id\">{id}</dc:identifier><dc:title>{title}</dc:title>{creator}<dc:language>{lang}</dc:language><dc:source>{src}</dc:source>{date}<meta property=\"dcterms:modified\">{modified}</meta></metadata><manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/><item id=\"chapter\" href=\"chapter.xhtml\" media-type=\"application/xhtml+xml\"/><item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>{pictures}</manifest><spine><itemref idref=\"chapter\"/></spine></package>",
        id = escape(b.identifier),
        title = escape(b.title),
        lang = escape(b.language),
        src = escape(b.source),
        modified = escape(b.modified),
    )
}

fn nav(b: &Book) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><title>Contents</title><meta charset=\"utf-8\"/></head><body><nav epub:type=\"toc\" id=\"toc\"><ol><li><a href=\"chapter.xhtml\">{}</a></li></ol></nav></body></html>",
        escape(b.title)
    )
}

const CONTAINER: &str = "<?xml version=\"1.0\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>";

pub fn write(b: &Book) -> Result<Vec<u8>, ClipError> {
    let err = |e: &dyn std::fmt::Display| ClipError::Epub(e.to_string());
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let fixed = DateTime::default();
    let stored = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(fixed);
    let deflated = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(fixed);
    let mut add = |name: &str, data: &[u8], opts: SimpleFileOptions| -> Result<(), ClipError> {
        zip.start_file(name, opts).map_err(|e| err(&e))?;
        zip.write_all(data).map_err(|e| err(&e))
    };
    add("mimetype", b"application/epub+zip", stored)?;
    add("META-INF/container.xml", CONTAINER.as_bytes(), deflated)?;
    add("OEBPS/content.opf", package(b).as_bytes(), deflated)?;
    add("OEBPS/nav.xhtml", nav(b).as_bytes(), deflated)?;
    add("OEBPS/chapter.xhtml", chapter(b).as_bytes(), deflated)?;
    add("OEBPS/style.css", STYLE.as_bytes(), deflated)?;
    for p in b.pictures {
        // a picture is already compressed
        add(&format!("OEBPS/{}", p.name), &p.bytes, stored)?;
    }
    Ok(zip.finish().map_err(|e| err(&e))?.into_inner())
}
