import re
from pathlib import Path

from PIL import Image
from PySide6.QtWidgets import QApplication

import liblevo.presentation.design_system as design_system_module
from liblevo.branding import APP_ICON_PATH, BRAND_DARK_LOGO_PATH, BRAND_LOGO_PATH
from liblevo.presentation.design_system import (
    COLORS,
    ThemeMode,
    apply_liblevo_theme,
    contrast_ratio,
    current_theme_mode,
    current_theme_preference,
)


def test_theme_can_follow_system_and_switch_without_restarting(qtbot) -> None:
    application = QApplication.instance()
    assert isinstance(application, QApplication)
    try:
        apply_liblevo_theme(application, ThemeMode.DARK)
        assert current_theme_mode() is ThemeMode.DARK
        assert current_theme_preference() is ThemeMode.DARK
        assert COLORS.canvas == "#172E36"
        assert COLORS.table_surface != COLORS.canvas
        assert COLORS.table_border != COLORS.table_separator

        apply_liblevo_theme(application, ThemeMode.LIGHT)
        assert current_theme_mode() is ThemeMode.LIGHT
        assert COLORS.canvas == "#F7F4EE"
        assert COLORS.table_surface != COLORS.table_header
        assert COLORS.table_border != COLORS.table_separator

        apply_liblevo_theme(application, ThemeMode.SYSTEM)
        assert current_theme_preference() is ThemeMode.SYSTEM
        assert current_theme_mode() in {ThemeMode.LIGHT, ThemeMode.DARK}
    finally:
        apply_liblevo_theme(application, ThemeMode.DARK)


def test_semantic_colours_meet_core_wcag_contrast_targets() -> None:
    application = QApplication.instance()
    assert isinstance(application, QApplication)
    try:
        for mode in (ThemeMode.LIGHT, ThemeMode.DARK):
            apply_liblevo_theme(application, mode)
            assert contrast_ratio(COLORS.text_primary, COLORS.canvas) >= 4.5
            assert contrast_ratio(COLORS.text_secondary, COLORS.canvas) >= 4.5
            assert contrast_ratio(COLORS.text_muted, COLORS.canvas) >= 4.5
            assert contrast_ratio(COLORS.text_inverse, COLORS.action_primary) >= 4.5
            assert contrast_ratio(COLORS.overlay_text, COLORS.overlay) >= 4.5
            assert contrast_ratio(COLORS.border, COLORS.surface) >= 3
            assert contrast_ratio(COLORS.focus_ring, COLORS.canvas) >= 3
            assert COLORS.divider != COLORS.border
            for status in (COLORS.info, COLORS.success, COLORS.warning, COLORS.error):
                assert contrast_ratio(status, COLORS.canvas) >= 4.5
    finally:
        apply_liblevo_theme(application, ThemeMode.DARK)


def test_each_theme_has_a_transparent_official_wordmark() -> None:
    variants = (
        (Path(BRAND_LOGO_PATH), ((23, 107, 99), (221, 239, 232), (23, 46, 54))),
        (Path(BRAND_DARK_LOGO_PATH), ((255, 255, 255), (221, 239, 232))),
    )
    for path, palette in variants:
        with Image.open(path).convert("RGBA") as logo:
            assert logo.getchannel("A").getextrema() == (0, 255)
            # Transparent edges may blend; solid interiors use the approved flat palette.
            assert all(
                any(
                    max(
                        abs(channel - expected)
                        for channel, expected in zip(pixel[:3], color, strict=True)
                    )
                    <= 1
                    for color in palette
                )
                for pixel in logo.get_flattened_data()
                if pixel[3] == 255
            )


def test_windows_icon_has_a_contrasting_book_on_teal_and_transparent_corners() -> None:
    with Image.open(APP_ICON_PATH).convert("RGBA") as icon:
        pixels = tuple(icon.get_flattened_data())
        assert icon.getchannel("A").getextrema() == (0, 255)
        assert all(icon.getpixel(point)[3] == 0 for point in ((0, 0), (1023, 0), (0, 1023)))
        assert sum(pixel == (23, 107, 99, 255) for pixel in pixels) > 200_000
        assert sum(pixel == (255, 255, 255, 255) for pixel in pixels) > 100_000
        assert all(
            red >= 23 and green >= 107 and blue >= 99
            for red, green, blue, alpha in pixels
            if alpha == 255
        )
        assert contrast_ratio("#FFFFFF", "#176B63") >= 4.5


def test_product_ui_colours_are_centralized_in_the_design_system() -> None:
    source_root = Path(__file__).parents[1] / "src" / "liblevo"
    allowed = {
        source_root / "presentation" / "design_system.py",
        # These colours are written into the exported EPUB, not the app UI.
        source_root / "epub_builder.py",
    }
    arbitrary_colours: list[str] = []
    for source in source_root.rglob("*.py"):
        if source in allowed:
            continue
        for match in re.finditer(r"#[0-9A-Fa-f]{3,8}\b", source.read_text(encoding="utf-8")):
            arbitrary_colours.append(f"{source.relative_to(source_root)}:{match.start()}")

    assert arbitrary_colours == []


def test_high_contrast_uses_the_native_palette_for_widgets_and_painters(
    qtbot,
    monkeypatch,
) -> None:
    application = QApplication.instance()
    assert isinstance(application, QApplication)
    monkeypatch.setattr(design_system_module, "is_high_contrast_enabled", lambda: True)

    try:
        apply_liblevo_theme(application, ThemeMode.LIGHT)

        assert application.styleSheet() == ""
        assert application.property("liblevoHighContrast") is True
        assert COLORS.canvas == application.palette().window().color().name().upper()
    finally:
        monkeypatch.setattr(design_system_module, "is_high_contrast_enabled", lambda: False)
        apply_liblevo_theme(application, ThemeMode.DARK)


def test_saved_theme_is_read_from_the_existing_profile(monkeypatch) -> None:
    calls: list[tuple[str, str]] = []

    class SavedSettings:
        def __init__(self, organization: str, application: str) -> None:
            calls.append((organization, application))

        def value(self, key: str, default: str) -> str:
            return "light" if key == "appearance/theme" else default

    monkeypatch.setattr(design_system_module, "QSettings", SavedSettings)

    assert design_system_module.preferred_theme_mode() is ThemeMode.LIGHT
    assert calls == [("Liblevo", "Liblevo")]
