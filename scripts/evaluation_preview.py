"""Bounded, passive local previews for the development review page."""

from __future__ import annotations

import base64
from html import escape
from io import BytesIO
from pathlib import Path
from urllib.parse import unquote, urlsplit

from lxml import etree, html
from markdown_it import MarkdownIt
from PIL import Image

_TAGS = frozenset(
    "p div span h1 h2 h3 h4 h5 h6 strong em b i u s del blockquote pre code br hr "
    "ul ol li table thead tbody tfoot tr th td caption sup sub dl dt dd a img".split()
)
_DROP = frozenset("script style iframe object embed form input button svg math template".split())


def _local_image(value: str, directory: Path) -> str | None:
    url = urlsplit(value)
    if url.scheme or url.netloc or "\\" in value:
        return None
    path = (directory / unquote(url.path)).resolve()
    if not path.is_relative_to(directory.resolve()) or not path.is_file():
        return None
    if path.suffix.lower() not in {".png", ".jpg", ".jpeg", ".webp", ".gif"}:
        return None
    if path.stat().st_size > 10 * 1024 * 1024:
        return None
    with Image.open(path) as image:
        if image.width * image.height > 20_000_000:
            return None
        image.thumbnail((1600, 1600))
        stream = BytesIO()
        image.convert("RGB").save(stream, format="PNG")
    return "data:image/png;base64," + base64.b64encode(stream.getvalue()).decode("ascii")


def passive_html(fragment: str, directory: Path) -> tuple[str, bool]:
    """Allow only structural HTML; never retain document CSS, URLs or active attributes."""
    container = html.fragment_fromstring(fragment or "<p></p>", create_parent="div")
    limited = False
    for node in tuple(container.iterdescendants()):
        if node.getparent() is None:
            continue
        tag = node.tag.lower() if isinstance(node.tag, str) else ""
        if not tag or tag in _DROP:
            if tag:
                limited = True
                node.drop_tree()
            else:
                parent = node.getparent()
                previous = node.getprevious()
                if previous is None:
                    parent.text = (parent.text or "") + (node.tail or "")
                else:
                    previous.tail = (previous.tail or "") + (node.tail or "")
                parent.remove(node)
            continue
        if tag not in _TAGS:
            node.drop_tag()
            limited = True
            continue
        attributes = dict(node.attrib)
        node.attrib.clear()
        if tag in {"td", "th"}:
            for key in ("colspan", "rowspan"):
                value = attributes.get(key, "")
                if value.isdecimal() and 1 <= int(value) <= 100:
                    node.set(key, value)
        if tag == "ol":
            value = attributes.get("start", "")
            if value.isdecimal() and len(value) <= 6:
                node.set("start", value)
        if tag == "img":
            try:
                source = _local_image(attributes.get("src", ""), directory)
            except (OSError, ValueError, Image.DecompressionBombError):
                source = None
            if source:
                node.set("src", source)
                node.set("alt", attributes.get("alt", ""))
            else:
                node.tag = "span"
                node.text = "[Imagen no disponible en esta vista]"
                limited = True
    return etree.tostring(container, encoding="unicode", method="html"), limited


def document_preview(path: Path) -> tuple[str, bool]:
    if path.suffix.lower() not in {".md", ".txt", ".html", ".htm"}:
        return "<p>Abre el archivo completo para revisar este formato.</p>", True
    with path.open("rb") as stream:
        data = stream.read(80001)
    text = data[:80000].decode("utf-8", errors="replace")
    if path.suffix.lower() == ".txt":
        return f'<div class="plain-text">{escape(text)}</div>', len(data) > 80000
    fragment = (
        MarkdownIt("commonmark", {"html": True}).enable(["table", "strikethrough"]).render(text)
        if path.suffix.lower() == ".md"
        else text
    )
    rendered, limited = passive_html(fragment, path.parent)
    return rendered, limited or len(data) > 80000


def original_preview(
    path: Path, pages: list[int] | None, view: Path, index: int
) -> tuple[str, bool]:
    if path.suffix.lower() != ".pdf":
        return document_preview(path)
    import pypdfium2 as pdfium

    parts = []
    with pdfium.PdfDocument(path) as document:
        first, last = pages or [1, len(document)]
        stop = min(last, first + 5)
        for number in range(first, stop + 1):
            page = document[number - 1]
            try:
                name = f"original-{index}-page-{number}.png"
                bitmap = page.render(scale=min(2.0, 1200 / max(page.get_width(), 1)))
                try:
                    bitmap.to_pil().save(view / name)
                finally:
                    bitmap.close()
            finally:
                page.close()
            parts.append(
                f"<figure><figcaption>Página {number} del PDF</figcaption>"
                f'<img src="{name}" alt="Página {number} del original"></figure>'
            )
    return "".join(parts), stop < last
