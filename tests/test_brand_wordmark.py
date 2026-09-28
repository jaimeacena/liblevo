"""Compare the outlined brand lettering with native Inter glyphs."""

import math

import pytest
from PIL import Image, ImageFilter
from PySide6.QtCore import QPointF, Qt
from PySide6.QtGui import QFont, QImage, QPainter, QPainterPath
from scripts.generate_brand_assets import _wordmark_path

from liblevo.branding import BRAND_TAGLINE
from liblevo.presentation.design_system import load_application_font


@pytest.mark.parametrize("text", ["liblevo", BRAND_TAGLINE])
def test_outlined_lettering_keeps_solid_strokes_and_intended_counters(qtbot, text) -> None:
    # Compare both strings at master scale so small-text hinting cannot hide cuts.
    size = 232
    family = load_application_font()
    font = QFont(family)
    font.setPixelSize(size)
    font.setWeight(QFont.Weight.DemiBold)
    font.setHintingPreference(QFont.HintingPreference.PreferNoHinting)
    path = _wordmark_path(family, text, size)
    bounds = path.boundingRect()
    dimensions = (math.ceil(bounds.width()) + 16, math.ceil(bounds.height()) + 16)

    def render(outline: QPainterPath | None) -> Image.Image:
        image = QImage(*dimensions, QImage.Format.Format_RGBA8888)
        image.fill(Qt.GlobalColor.transparent)
        painter = QPainter(image)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.translate(8 - bounds.left(), 8 - bounds.top())
        if outline is None:
            painter.setPen(Qt.GlobalColor.black)
            painter.setFont(font)
            painter.drawText(QPointF(0, 0), text)
        else:
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(Qt.GlobalColor.black)
            painter.drawPath(outline)
        painter.end()
        return Image.frombytes("RGBA", dimensions, bytes(image.constBits())).getchannel("A")

    reference = render(None)
    actual = render(path)
    # Ignore the two-pixel rasterization boundary, but detect cuts inside strokes.
    solid = reference.filter(ImageFilter.MinFilter(5))
    empty = reference.filter(ImageFilter.MaxFilter(5))
    unexpected_cuts = sum(
        expected > 250 and observed < 240
        for expected, observed in zip(
            solid.get_flattened_data(), actual.get_flattened_data(), strict=True
        )
    )
    filled_counters = sum(
        expected == 0 and observed > 15
        for expected, observed in zip(
            empty.get_flattened_data(), actual.get_flattened_data(), strict=True
        )
    )

    assert unexpected_cuts == 0
    assert filled_counters == 0
