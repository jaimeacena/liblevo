"""Stable application errors exposed by the local processing core."""


class LiblevoError(Exception):
    """Base class for expected Liblevo failures."""


class RequestValidationError(LiblevoError):
    """The requested operation or one of its paths is invalid."""


class ConversionError(LiblevoError):
    """A supported local document could not be converted to Markdown."""


class OutputWriteError(LiblevoError):
    """A Markdown result could not be written safely."""


class FinalIntegrityError(LiblevoError):
    """The staged final artifact does not match the validated content."""


class EarlyCheckError(LiblevoError):
    """A representative sample found a material risk before the full run."""


class ReviewUnavailableError(LiblevoError):
    """A Markdown file cannot be loaded safely into the in-app review."""


class ProcessingCancelledError(LiblevoError):
    """The user requested cancellation at a safe processing boundary."""


class SettingsError(LiblevoError):
    """Application settings could not be loaded, validated or saved."""


class LocalModelUnavailableError(LiblevoError):
    """Ollama or the selected local model could not be reached."""


class ImprovementError(LiblevoError):
    """A local model response was invalid or unsafe to publish."""


class TranslationError(LiblevoError):
    """A free offline translation could not be completed safely."""


class UnexpectedProcessingError(LiblevoError):
    """An unexpected failure was recorded without exposing private document data."""
