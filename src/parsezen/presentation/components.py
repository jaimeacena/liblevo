"""Small accessible presentation primitives shared by Parsezen workflows."""

from __future__ import annotations

from PySide6.QtCore import QEvent, QPoint, QRectF, QSize, Qt, Signal
from PySide6.QtGui import QColor, QFocusEvent, QPainter, QPaintEvent, QPen, QResizeEvent
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QFrame,
    QGridLayout,
    QLabel,
    QPushButton,
    QScrollArea,
    QSizePolicy,
    QWidget,
)

from parsezen.presentation.design_system import COLORS, CONTROLS, RADII, SPACING
from parsezen.translation_quality import TARGET_LANGUAGE_CODES


class BookLanguageSelector(QComboBox):
    """Display language names while preserving the book's exact language tag."""

    def __init__(self, language: str, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setAccessibleName("Idioma del libro")
        self.setMinimumWidth(0)
        self.setSizeAdjustPolicy(QComboBox.SizeAdjustPolicy.AdjustToMinimumContentsLengthWithIcon)
        self.addItem("Sin especificar", "und")
        for name, code in TARGET_LANGUAGE_CODES.items():
            self.addItem(name, code)
        index = self.findData(language)
        if index < 0:
            # Existing regional/other tags must survive a metadata-only edit unchanged.
            base = language.split("-")[0].lower()
            language_name = next(
                (name for name, code in TARGET_LANGUAGE_CODES.items() if code == base), None
            )
            self.addItem(f"{language_name} ({language})" if language_name else language, language)
            index = self.count() - 1
        self.setCurrentIndex(index)

    def language_code(self) -> str:
        return str(self.currentData())


class ElidedLabel(QLabel):
    """Keep full accessible text while deliberately eliding constrained headings."""

    def __init__(self, text: str, parent: QWidget | None = None) -> None:
        super().__init__(text, parent)
        self.full_text = text
        self.setTextFormat(Qt.TextFormat.PlainText)
        self.setAccessibleName(text)
        self.setToolTip(text)
        self.setMinimumWidth(0)
        self.setSizePolicy(QSizePolicy.Policy.Ignored, QSizePolicy.Policy.Preferred)

    def resizeEvent(self, event: QResizeEvent) -> None:  # noqa: N802
        super().resizeEvent(event)
        self._elide()

    def changeEvent(self, event: QEvent) -> None:  # noqa: N802
        super().changeEvent(event)
        if hasattr(self, "full_text"):
            self._elide()

    def _elide(self) -> None:
        text = self.fontMetrics().elidedText(
            self.full_text, Qt.TextElideMode.ElideMiddle, self.contentsRect().width()
        )
        if text != self.text():
            super().setText(text)


class Switch(QCheckBox):
    """Keyboard-operable, screen-reader-labelled binary switch."""

    toggledByUser = Signal(bool)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setFixedSize(48, 32)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self.setAccessibleDescription("Interruptor; pulsa Espacio para cambiarlo.")
        self.clicked.connect(self.toggledByUser)

    def sizeHint(self) -> QSize:  # noqa: N802
        return QSize(48, 32)

    def hitButton(self, position: QPoint) -> bool:  # noqa: N802
        return self.isEnabled() and self.rect().contains(position)

    def focusInEvent(self, event: QFocusEvent) -> None:  # noqa: N802
        super().focusInEvent(event)
        self.update()

    def focusOutEvent(self, event: QFocusEvent) -> None:  # noqa: N802
        super().focusOutEvent(event)
        self.update()

    def paintEvent(self, _event: QPaintEvent) -> None:  # noqa: N802
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        track = QRectF(4, 6, 40, 20)
        if not self.isEnabled():
            track_color = QColor(COLORS.surface_subtle)
            border_color = QColor(COLORS.border)
            knob_color = QColor(COLORS.text_disabled)
        elif self.isChecked():
            track_color = QColor(COLORS.action_primary)
            border_color = QColor(COLORS.action_primary)
            knob_color = QColor(COLORS.text_inverse)
        else:
            track_color = QColor(COLORS.surface)
            border_color = QColor(COLORS.border_strong)
            knob_color = QColor(COLORS.text_secondary)
        painter.setPen(QPen(border_color, 1.5))
        painter.setBrush(track_color)
        painter.drawRoundedRect(track, 10, 10)
        knob_left = 26 if self.isChecked() else 7
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(knob_color)
        painter.drawEllipse(QRectF(knob_left, 9, 14, 14))
        if self.hasFocus():
            painter.setBrush(Qt.BrushStyle.NoBrush)
            painter.setPen(QPen(QColor(COLORS.focus_ring), 2))
            painter.drawRoundedRect(QRectF(1, 1, 46, 30), RADII.medium, RADII.medium)


class StatusMessage(QFrame):
    """Reusable inline feedback with a clear message and optional recovery action."""

    actionRequested = Signal()
    secondaryActionRequested = Signal()

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("statusMessage")
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Minimum)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        layout = QGridLayout(self)
        self.layout_grid = layout
        layout.setContentsMargins(SPACING.md, SPACING.sm, SPACING.md, SPACING.sm)
        layout.setSpacing(SPACING.sm)
        self.icon = QLabel(self)
        self.icon.setObjectName("statusMessageIcon")
        layout.addWidget(self.icon, 0, 0)
        self.message = QLabel(self)
        self.message.setWordWrap(True)
        self.message.setMinimumWidth(0)
        self.message.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
        self.message.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        layout.addWidget(self.message, 0, 1)
        layout.setColumnStretch(1, 1)
        self.action = QPushButton(self)
        self.action.clicked.connect(self.actionRequested)
        self.action.hide()
        layout.addWidget(self.action, 0, 2)
        self.secondary_action = QPushButton(self)
        self.secondary_action.clicked.connect(self.secondaryActionRequested)
        self.secondary_action.hide()
        layout.addWidget(self.secondary_action, 0, 3)
        self._compact = False
        self.hide()

    def show_message(
        self,
        text: str,
        *,
        tone: str = "info",
        action_label: str | None = None,
        secondary_action_label: str | None = None,
    ) -> None:
        symbols = {
            "info": "i",
            "success": "✓",
            "warning": "!",
            "error": "!",
        }
        self.setProperty("tone", tone)
        self.style().unpolish(self)
        self.style().polish(self)
        self.icon.setText(symbols.get(tone, "i"))
        self.message.setText(text)
        self.setAccessibleName(text)
        self.action.setText(action_label or "")
        self.action.setVisible(bool(action_label))
        self.secondary_action.setText(secondary_action_label or "")
        self.secondary_action.setVisible(bool(secondary_action_label))
        self.show()
        self.layout_grid.invalidate()
        self.updateGeometry()

    def set_compact_mode(self, compact: bool) -> None:
        if compact == self._compact:
            return
        self._compact = compact
        for widget in (self.icon, self.message, self.action, self.secondary_action):
            self.layout_grid.removeWidget(widget)
        self.layout_grid.addWidget(self.icon, 0, 0)
        self.layout_grid.addWidget(self.message, 0, 1, 1, 3)
        if compact:
            self.layout_grid.addWidget(self.action, 1, 1, 1, 3)
            self.layout_grid.addWidget(self.secondary_action, 2, 1, 1, 3)
        else:
            self.layout_grid.addWidget(self.action, 0, 2)
            self.layout_grid.addWidget(self.secondary_action, 0, 3)


class HorizontalToolStrip(QScrollArea):
    """One-row toolbar that remains keyboard and touch reachable on small screens."""

    def __init__(self, content: QWidget, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("horizontalToolStrip")
        self.setWidgetResizable(True)
        self.setFrameShape(QFrame.Shape.NoFrame)
        self.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        self.setMinimumHeight(CONTROLS.compact_height + SPACING.sm)
        self.setWidget(content)
