"""Lightweight final confirmation shown before every EPUB publication."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from PySide6.QtCore import Qt
from PySide6.QtGui import QResizeEvent
from PySide6.QtWidgets import (
    QBoxLayout,
    QDialog,
    QFormLayout,
    QFrame,
    QLabel,
    QLayout,
    QLineEdit,
    QMessageBox,
    QPushButton,
    QScrollArea,
    QSizePolicy,
    QVBoxLayout,
    QWidget,
)

from liblevo.application.artifact_repository import ArtifactRepository
from liblevo.application.book_editor import BookEditor
from liblevo.domain.books import BookDocument
from liblevo.presentation.components import BookLanguageSelector
from liblevo.presentation.design_system import SPACING


class EpubConfirmationDialog(QDialog):
    """Confirm essential book facts without forcing the full chapter editor."""

    def __init__(
        self,
        book: BookDocument,
        artifacts: ArtifactRepository,
        *,
        job_id: str,
        preserved_review_chunks: int = 0,
        destination: Path | None = None,
        save_draft: Callable[[BookDocument], None] | None = None,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self._book = book
        self._artifacts = artifacts
        self._job_id = job_id
        self._save_draft = save_draft
        self._open_editor_requested = False
        self._saved_for_later = False
        self.setWindowTitle("Confirmar libro EPUB · Liblevo")
        self.setObjectName("epubConfirmationDialog")
        self.resize(720, 620)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(SPACING.lg, SPACING.lg, SPACING.lg, SPACING.lg)
        layout.setSpacing(SPACING.md)
        self.content_scroll = QScrollArea(self)
        self.content_scroll.setWidgetResizable(True)
        self.content_scroll.setFrameShape(QFrame.Shape.NoFrame)
        self.content_scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self.content = QWidget()
        self.content.setMaximumWidth(760)
        self.content_scroll.setAlignment(Qt.AlignmentFlag.AlignHCenter)
        content_layout = QVBoxLayout(self.content)
        content_layout.setSizeConstraint(QLayout.SizeConstraint.SetMinimumSize)
        content_layout.setContentsMargins(0, 0, SPACING.sm, 0)
        content_layout.setSpacing(SPACING.md)
        self.content_scroll.setWidget(self.content)
        layout.addWidget(self.content_scroll, 1)
        title = QLabel("Confirma tu EPUB", self.content)
        title.setObjectName("dialogTitle")
        title.setWordWrap(True)
        content_layout.addWidget(title)
        explanation = QLabel(
            "Puedes corregir los datos básicos aquí. El contenido y los recursos no se "
            "modifican salvo que abras el editor completo.",
            self.content,
        )
        explanation.setObjectName("confirmationHelp")
        explanation.setWordWrap(True)
        content_layout.addWidget(explanation)

        if preserved_review_chunks:
            warning = QLabel(
                "Parte de la revisión con IA no pudo completarse. Se conservó el texto "
                f"anterior en {preserved_review_chunks} fragmentos. Puedes comprobarlo "
                "en el editor antes de generar el EPUB.",
                self.content,
            )
            warning.setObjectName("reviewIncompleteWarning")
            warning.setWordWrap(True)
            content_layout.addWidget(warning)

        form = QFormLayout()
        form.setRowWrapPolicy(QFormLayout.RowWrapPolicy.WrapAllRows)
        form.setVerticalSpacing(SPACING.sm)
        self.title_input = QLineEdit(book.metadata.title, self)
        self.author_input = QLineEdit(book.metadata.author or "", self)
        self.title_input.setAccessibleName("Título del libro")
        self.author_input.setAccessibleName("Autor del libro")
        self.language_input = BookLanguageSelector(book.metadata.language, self)
        self.cover_value = QLabel(
            "Incluida" if book.cover_resource_id is not None else "Sin portada",
            self,
        )
        self.chapter_value = QLabel(str(len(book.spine)), self)
        form.addRow("Título", self.title_input)
        form.addRow("Autor", self.author_input)
        form.addRow("Idioma", self.language_input)
        form.addRow("Portada", self.cover_value)
        form.addRow("Capítulos", self.chapter_value)
        content_layout.addLayout(form)

        note = QLabel(
            "Para cambiar la portada, reorganizar capítulos o editar el contenido, abre el "
            "editor completo.",
            self,
        )
        note.setObjectName("confirmationHelp")
        note.setWordWrap(True)
        content_layout.addWidget(note)
        destination_text = (
            f"Se guardará en este equipo: {destination}"
            if destination is not None
            else "Se guardará en este equipo, en la carpeta de destino elegida."
        )
        self.destination_label = QLabel(destination_text, self.content)
        self.destination_label.setObjectName("confirmationHelp")
        self.destination_label.setWordWrap(True)
        self.destination_label.setMinimumWidth(0)
        content_layout.addWidget(self.destination_label)
        content_layout.addStretch(1)

        self.actions_layout = QBoxLayout(QBoxLayout.Direction.LeftToRight)
        self.save_later_button = QPushButton("Guardar y salir", self)
        self.save_later_button.setToolTip(
            "Guardar el borrador para continuar después; no genera el archivo final."
        )
        self.editor_button = QPushButton("Abrir editor completo", self)
        self.publish_button = QPushButton("Generar EPUB", self)
        self.publish_button.setObjectName("primaryAction")
        for button in (self.save_later_button, self.editor_button, self.publish_button):
            button.setSizePolicy(QSizePolicy.Policy.Minimum, QSizePolicy.Policy.Fixed)
            self.actions_layout.addWidget(button)
        layout.addLayout(self.actions_layout)

        self.save_later_button.clicked.connect(self._save_and_close)
        self.editor_button.clicked.connect(self._open_editor)
        self.publish_button.clicked.connect(self._publish)

    def resizeEvent(self, event: QResizeEvent) -> None:  # noqa: N802
        super().resizeEvent(event)
        needed = (
            sum(
                button.sizeHint().width()
                for button in (self.save_later_button, self.editor_button, self.publish_button)
            )
            + SPACING.md * 4
        )
        self.actions_layout.setDirection(
            QBoxLayout.Direction.TopToBottom
            if event.size().width() < needed
            else QBoxLayout.Direction.LeftToRight
        )

    @property
    def book(self) -> BookDocument:
        return self._book

    @property
    def open_editor_requested(self) -> bool:
        return self._open_editor_requested

    @property
    def saved_for_later(self) -> bool:
        return self._saved_for_later

    def reject(self) -> None:
        """Closing the page preserves the draft instead of publishing it."""

        self._save_and_close()

    def _update_metadata(self) -> bool:
        try:
            self._book = BookEditor(
                self._book,
                self._artifacts,
                job_id=self._job_id,
            ).update_metadata(
                title=self.title_input.text(),
                author=self.author_input.text(),
                language=self.language_input.language_code(),
            )
        except (ValueError, OSError) as exc:
            QMessageBox.warning(self, "Datos no válidos", str(exc))
            return False
        return True

    def _publish(self) -> None:
        if self._update_metadata() and self._persist_draft():
            self._saved_for_later = False
            self.accept()

    def _open_editor(self) -> None:
        if self._update_metadata() and self._persist_draft():
            self._open_editor_requested = True
            self._saved_for_later = False
            self.accept()

    def _save_and_close(self) -> None:
        if not self._update_metadata() or not self._persist_draft():
            return
        self._saved_for_later = True
        super().reject()

    def _persist_draft(self) -> bool:
        try:
            if self._save_draft is not None:
                self._save_draft(self._book)
        except (OSError, RuntimeError, ValueError) as exc:
            QMessageBox.warning(self, "No se pudo guardar el borrador", str(exc))
            return False
        return True
