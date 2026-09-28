"""Generate Liblevo vector wordmarks and Windows icons from the approved book mark.

The wordmark uses outlines from bundled Inter, without external fonts or services.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

from PIL import Image
from PySide6.QtCore import QByteArray, QRectF, QSize, Qt
from PySide6.QtGui import QColor, QFont, QFontDatabase, QImage, QPainter, QPainterPath
from PySide6.QtSvg import QSvgGenerator, QSvgRenderer
from PySide6.QtWidgets import QApplication

from liblevo import APP_DISPLAY_NAME
from liblevo.branding import BRAND_TAGLINE
from liblevo.presentation.design_system import (
    BRAND_INK,
    BRAND_MINT,
    BRAND_PAPER,
    BRAND_TEAL,
    BRAND_WHITE,
)

ROOT = Path(__file__).resolve().parents[1]
BRANDING_ROOT = ROOT / "assets" / "branding"
SYMBOL_SOURCE_PATH = BRANDING_ROOT / "masters" / "liblevo-symbol.svg"
FONT_PATH = ROOT / "assets" / "fonts" / "Inter.ttf"
GENERATED_ROOT = BRANDING_ROOT / "generated"
ICON_SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256)


def _symbol_svg(primary: str, page: str) -> bytes:
    source = SYMBOL_SOURCE_PATH.read_text(encoding="utf-8")
    return source.replace(BRAND_TEAL, primary).replace(BRAND_MINT, page).encode("utf-8")


def _wordmark_path(family: str, text: str, size: int) -> QPainterPath:
    font = QFont(family)
    font.setPixelSize(size)
    font.setWeight(QFont.Weight.DemiBold)
    path = QPainterPath()
    # Variable-font contours overlap; preserve their union and the intended counters.
    path.setFillRule(Qt.FillRule.WindingFill)
    path.addText(0, 0, font, text)
    return path


def _write_logo(family: str, *, dark: bool, tagline: bool = False) -> Path:
    wordmark = _wordmark_path(family, "liblevo", 232)
    bounds = wordmark.boundingRect()
    width = round(310 + bounds.width() + 16)
    height = 410 if tagline else 310
    name = (
        "liblevo-logo-tagline.svg" if tagline else f"liblevo-logo-{'dark' if dark else 'light'}.svg"
    )
    path = GENERATED_ROOT / name
    generator = QSvgGenerator()
    generator.setFileName(str(path))
    generator.setSize(QSize(width, height))
    generator.setViewBox(QRectF(0, 0, width, height))
    generator.setTitle(f"{APP_DISPLAY_NAME} · {BRAND_TAGLINE}")
    generator.setDescription("L con forma de libro; Inter semibold convertido a contornos.")
    painter = QPainter(generator)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    painter.setPen(Qt.PenStyle.NoPen)
    QSvgRenderer(QByteArray(_symbol_svg(BRAND_WHITE if dark else BRAND_TEAL, BRAND_MINT))).render(
        painter, QRectF(0, 0, 256, 310)
    )
    painter.setPen(Qt.PenStyle.NoPen)
    painter.setBrush(QColor(BRAND_WHITE if dark else BRAND_INK))
    painter.translate(310 - bounds.left(), (310 - bounds.height()) / 2 - bounds.top())
    painter.drawPath(wordmark)
    painter.resetTransform()
    if tagline:
        slogan = _wordmark_path(family, BRAND_TAGLINE, 50)
        slogan_bounds = slogan.boundingRect()
        painter.translate(310 - slogan_bounds.left(), 355 - slogan_bounds.top())
        painter.drawPath(slogan)
    painter.end()
    # The flat mark has no strokes; discard outlines added by the SVG painter round-trip.
    document = path.read_text(encoding="utf-8")
    document = document.replace('stroke="black"', 'stroke="none"').replace(
        'stroke="#000000"', 'stroke="none"'
    )
    path.write_text(document, encoding="utf-8", newline="\n")
    return path


def _render_svg(source: Path, target: Path, *, height: int) -> None:
    renderer = QSvgRenderer(str(source))
    if not renderer.isValid():
        raise ValueError(f"Recurso vectorial inválido: {source.name}")
    bounds = renderer.viewBoxF()
    width = round(height * bounds.width() / bounds.height())
    image = QImage(width * 3, height * 3, QImage.Format.Format_ARGB32_Premultiplied)
    image.fill(Qt.GlobalColor.transparent)
    painter = QPainter(image)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    painter.setPen(Qt.PenStyle.NoPen)
    renderer.render(painter)
    painter.end()
    image = image.scaled(
        width,
        height,
        Qt.AspectRatioMode.IgnoreAspectRatio,
        Qt.TransformationMode.SmoothTransformation,
    )
    if not image.save(str(target)):
        raise OSError(f"No se pudo guardar {target.name}")


def _write_app_icon() -> Path:
    source = _symbol_svg(BRAND_WHITE, BRAND_WHITE).decode("utf-8")
    paths = source[source.index("  <path") : source.rindex("</svg>")]
    path = GENERATED_ROOT / "liblevo-app-icon.svg"
    path.write_text(
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" stroke="none">\n'
        f"  <title>{APP_DISPLAY_NAME}</title>\n"
        f'  <rect x="48" y="48" width="928" height="928" rx="160" fill="{BRAND_TEAL}" '
        'stroke="none"/>\n'
        '  <g transform="translate(220 155) scale(2.3)">\n'
        f"{paths}  </g>\n</svg>\n",
        encoding="utf-8",
    )
    return path


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def generate() -> None:
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    application = QApplication.instance() or QApplication([])
    font_id = QFontDatabase.addApplicationFont(str(FONT_PATH))
    families = QFontDatabase.applicationFontFamilies(font_id)
    if not families:
        raise ValueError("No se pudo cargar el maestro tipográfico Inter.")
    GENERATED_ROOT.mkdir(parents=True, exist_ok=True)
    paths: list[Path] = []
    for dark in (False, True):
        logo = _write_logo(families[0], dark=dark)
        paths.append(logo)
        for height, suffix in ((96, ""), (192, "@2x")):
            target = logo.with_name(logo.stem + suffix + ".png")
            _render_svg(logo, target, height=height)
            paths.append(target)
    tagline = _write_logo(families[0], dark=False, tagline=True)
    paths.append(tagline)
    readme = GENERATED_ROOT / "liblevo-readme.png"
    _render_svg(tagline, readme, height=164)
    paths.append(readme)
    monochrome = GENERATED_ROOT / "liblevo-symbol-mono.svg"
    monochrome.write_bytes(_symbol_svg(BRAND_INK, BRAND_INK))
    paths.append(monochrome)
    app_icon = _write_app_icon()
    paths.append(app_icon)
    symbol = GENERATED_ROOT / "liblevo-symbol-1024.png"
    _render_svg(app_icon, symbol, height=1024)
    paths.append(symbol)
    icon = GENERATED_ROOT / "liblevo-app-icon.ico"
    with Image.open(symbol) as image:
        image.save(icon, sizes=[(size, size) for size in ICON_SIZES])
    paths.append(icon)
    # Keep generated vectors clean and hash the exact LF bytes stored by Git.
    for path in paths:
        if path.suffix == ".svg":
            document = "\n".join(
                line.rstrip() for line in path.read_text(encoding="utf-8").splitlines()
            )
            path.write_text(document + "\n", encoding="utf-8", newline="\n")
    manifest = {
        "brand": APP_DISPLAY_NAME,
        "tagline": BRAND_TAGLINE,
        "source": SYMBOL_SOURCE_PATH.relative_to(ROOT).as_posix(),
        "source_sha256": _sha256(SYMBOL_SOURCE_PATH),
        "font": FONT_PATH.relative_to(ROOT).as_posix(),
        "font_sha256": _sha256(FONT_PATH),
        "palette": {
            "teal": BRAND_TEAL,
            "ink": BRAND_INK,
            "paper": BRAND_PAPER,
            "mint": BRAND_MINT,
            "white": BRAND_WHITE,
        },
        "outputs": {path.name: {"sha256": _sha256(path)} for path in paths},
    }
    (GENERATED_ROOT / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    # Keep the Qt application alive until all painters and renderers have finished.
    del application


if __name__ == "__main__":
    generate()
