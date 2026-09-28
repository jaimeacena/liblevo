"""Release regressions reproduced by the September visual review."""

from datetime import UTC, datetime
from pathlib import Path

import pytest
from PySide6.QtCore import QPoint, QRect, Qt
from PySide6.QtGui import QPalette
from PySide6.QtWidgets import QApplication, QLabel

from liblevo.application.book_editor import create_book_from_markdown
from liblevo.component_readiness import ReadinessStatus
from liblevo.domain.jobs import DocumentJob, DocumentSource, JobConfiguration
from liblevo.domain.outcomes import OutcomeSummary
from liblevo.epub_builder import EpubBookMetadata
from liblevo.infrastructure.artifact_store import ArtifactStore
from liblevo.local_ai_policy import ComponentCapability
from liblevo.local_models import OllamaStatus
from liblevo.presentation.activity_view import ActivityView
from liblevo.presentation.component_setup import ComponentSetupDialog
from liblevo.presentation.design_system import ThemeMode, apply_liblevo_theme, contrast_ratio
from liblevo.presentation.epub_confirmation_dialog import EpubConfirmationDialog
from liblevo.presentation.job_configuration_dialog import JobConfigurationDialog
from liblevo.presentation.workspace import LiblevoWorkspace
from liblevo.recent_activity import RecentJob, RecentJobStatus


def _confirmation(tmp_path: Path, *, warning: bool = True, language: str = "es"):
    store = ArtifactStore(tmp_path / "artifacts")
    book = create_book_from_markdown(
        "# Capítulo\n\nContenido de prueba.",
        (),
        EpubBookMetadata("Un libro para comprobar la interfaz", language),
        store,
        job_id="job",
    )
    return EpubConfirmationDialog(
        book, store, job_id="job", preserved_review_chunks=2 if warning else 0
    )


def _embed(qtbot, view, width, height):
    shell = LiblevoWorkspace()
    qtbot.addWidget(shell)
    view.setWindowFlags(Qt.WindowType.Widget)
    shell.resize(width, height)
    shell.show()
    shell.show_internal_view(view, "Confirmar EPUB")
    QApplication.processEvents()
    return shell


@pytest.mark.parametrize("theme", [ThemeMode.LIGHT, ThemeMode.DARK])
@pytest.mark.parametrize("size", [(320, 520), (320, 720), (768, 600), (911, 520)])
def test_confirmation_keeps_readable_actions_and_scrollable_fields(qtbot, tmp_path, theme, size):
    apply_liblevo_theme(QApplication.instance(), theme)
    view = _confirmation(tmp_path)
    shell = _embed(qtbot, view, *size)
    buttons = (view.save_later_button, view.editor_button, view.publish_button)
    rects = [QRect(b.mapTo(shell, QPoint()), b.size()) for b in buttons]
    for button, rect in zip(buttons, rects, strict=True):
        assert shell.rect().contains(rect)
        assert button.width() >= button.fontMetrics().horizontalAdvance(button.text()) + 20
        assert button.height() >= 32
    assert all(not a.intersects(b) for i, a in enumerate(rects) for b in rects[i + 1 :])
    assert view.content_scroll.horizontalScrollBar().maximum() == 0
    assert view.title_input.geometry().bottom() < view.author_input.geometry().top()
    for label in view.content.findChildren(QLabel):
        if label.wordWrap() and label.isVisible():
            assert label.height() >= label.heightForWidth(label.width())
    view.content_scroll.ensureWidgetVisible(view.destination_label)
    QApplication.processEvents()
    assert view.destination_label.visibleRegion().boundingRect().height() > 0


@pytest.mark.parametrize(
    "initial,target", [(ThemeMode.DARK, ThemeMode.LIGHT), (ThemeMode.LIGHT, ThemeMode.DARK)]
)
def test_open_confirmation_remains_legible_after_live_theme_change(
    qtbot, tmp_path, initial, target
):
    app = QApplication.instance()
    apply_liblevo_theme(app, initial)
    view = _confirmation(tmp_path)
    shell = _embed(qtbot, view, 1000, 700)
    apply_liblevo_theme(app, target)
    for widget in app.allWidgets():
        refresh = getattr(widget, "apply_theme", None)
        if callable(refresh):
            refresh()
    QApplication.processEvents()
    background = view.grab().toImage().pixelColor(2, 2).name()
    label = view.content.findChild(QLabel, "dialogTitle")
    foreground = label.palette().color(QPalette.ColorRole.WindowText).name()
    assert contrast_ratio(foreground, background) >= 4.5
    assert shell.current_internal_widget is view


@pytest.mark.parametrize("size", [(320, 520), (320, 720), (768, 600)])
def test_result_summary_exposes_all_details_without_hiding_actions(qtbot, tmp_path, size):
    result = tmp_path / "resultado.epub"
    result.touch()
    summary = OutcomeSummary(
        output_format="EPUB",
        translation_issues=2,
        translation_checked_blocks=20,
        translation_reviewed_blocks=16,
        translation_unreviewed_blocks=4,
        ai_review_recommended=True,
        ai_review_blocks=2,
        ai_review_signals=3,
        integrity_verified=True,
        duration_seconds=600,
    )
    view = ActivityView(
        (
            RecentJob(
                tmp_path / "Documento con nombre largo.pdf",
                RecentJobStatus.COMPLETED,
                datetime.now(UTC),
                result,
                summary,
            ),
        ),
        allow_clear=False,
    )
    shell = _embed(qtbot, view, *size)
    view.summary_details_button.click()
    QApplication.processEvents()
    assert view.details_scroll.horizontalScrollBar().maximum() == 0
    qtbot.wait(50)
    assert not view.history_panel.isVisible()
    for button in (view.open_button, view.folder_button):
        assert shell.rect().contains(QRect(button.mapTo(shell, QPoint()), button.size()))
        assert button.width() >= button.fontMetrics().horizontalAdvance(button.text()) + 20
    assert not view.open_button.geometry().intersects(view.folder_button.geometry())
    view.details_scroll.verticalScrollBar().setValue(
        view.details_scroll.verticalScrollBar().maximum()
    )
    QApplication.processEvents()
    assert "Tiempo automático" in view.details_summary.text()
    assert (
        view.details_summary.visibleRegion().boundingRect().bottom()
        >= view.details_summary.height() - 1
    )


def test_language_selector_preserves_existing_regional_tag(qtbot, tmp_path):
    view = _confirmation(tmp_path, language="es-MX")
    qtbot.addWidget(view)
    assert view.language_input.currentText() == "Español (es-mx)"
    view.title_input.setText("Título corregido")
    view.publish_button.click()
    assert view.book.metadata.language == "es-mx"


@pytest.mark.parametrize("kind", ["configuration", "setup"])
@pytest.mark.parametrize("theme", [ThemeMode.LIGHT, ThemeMode.DARK])
def test_short_configuration_and_setup_scroll_without_overlapping_cards(
    qtbot, tmp_path, kind, theme
):
    apply_liblevo_theme(QApplication.instance(), theme)
    if kind == "configuration":
        source = tmp_path / "document.pdf"
        source.write_bytes(b"test")
        job = DocumentJob.create(DocumentSource.inspect(source), JobConfiguration(), order=0)
        view = JobConfigurationDialog(
            job, embedded=True, default_ai_model="local-test", ollama_status=OllamaStatus.READY
        )
        view._set_translation_language("es")
        cards = (view.markdown_card, view.epub_card)
        last = view.save_status
    else:
        view = ComponentSetupDialog(
            states={
                ComponentCapability.TRANSLATION: ReadinessStatus.PREPARED,
                ComponentCapability.REVIEW: ReadinessStatus.PREPARED,
            }
        )
        cards = tuple(view.cards.values())
        last = view.refresh_button
    _embed(qtbot, view, 320, 520)
    qtbot.wait(50)
    scroll = view.content_scroll
    assert scroll.horizontalScrollBar().maximum() == 0
    assert scroll.widget().width() <= scroll.viewport().width()
    assert scroll.verticalScrollBar().maximum() > 0
    first = QRect(cards[0].mapTo(scroll.widget(), QPoint()), cards[0].size())
    second = QRect(cards[1].mapTo(scroll.widget(), QPoint()), cards[1].size())
    assert not first.intersects(second)
    scroll.ensureWidgetVisible(last)
    qtbot.wait(50)
    assert last.visibleRegion().boundingRect() == last.rect()
