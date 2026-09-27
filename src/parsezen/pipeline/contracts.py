"""Immutable contracts shared by the preparation, transformation and publication stages."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import TYPE_CHECKING

from parsezen.domain.jobs import MarkdownOrganization
from parsezen.domain.process_lifecycle import ProcessStage
from parsezen.processing_metrics import BatchTelemetry
from parsezen.workflow import OutputFormat

if TYPE_CHECKING:
    from parsezen.document_model import ConvertedResource
    from parsezen.domain.execution_plan import ExecutionPlan
    from parsezen.epub_builder import EpubBookMetadata
    from parsezen.final_integrity import FinalIntegrityReport
    from parsezen.glossary import GlossaryEntry
    from parsezen.improvement_contracts import ImprovementMode
    from parsezen.pdf_conversion import PdfPageRange, PdfQualityReport
    from parsezen.revision import RevisionDraft
    from parsezen.semantic_blocks import SemanticDocument
    from parsezen.translation_quality import LinguisticReviewCoverage, TranslationQualityReport


@dataclass(frozen=True, slots=True)
class StageTelemetry:
    """Aggregate time spent in one observable processing stage."""

    stage: ProcessStage
    duration_ms: int
    visits: int


@dataclass(frozen=True, slots=True)
class ProcessTelemetry:
    """Privacy-safe timings for one completed local transformation."""

    total_duration_ms: int
    stages: tuple[StageTelemetry, ...]
    batches: BatchTelemetry = BatchTelemetry()


@dataclass(frozen=True, slots=True)
class ProcessSourceRequest:
    """Source inputs projected for batch validation."""

    path: Path
    convert_to_markdown: bool
    pdf_page_range: PdfPageRange | None
    force_pdf_ocr: bool
    size_bytes: int | None
    modified_ns: int | None
    content_sha256: str | None
    identity_verified: bool


@dataclass(frozen=True, slots=True)
class ProcessPublicationRequest:
    """Publication inputs projected for batch validation."""

    output_directory: Path | None
    output_format: OutputFormat
    image_output_directory: Path | None
    epub_title: str | None
    epub_author: str | None
    epub_cover_path: Path | None
    include_images: bool
    preserve_styles: bool
    markdown_organization: MarkdownOrganization
    markdown_include_metadata: bool
    markdown_include_page_references: bool
    epub_first_page_cover: bool
    epub_remove_cover: bool


@dataclass(frozen=True, slots=True)
class ProcessRequest:
    """Stable facade request for one local document pipeline run."""

    source_path: Path
    convert_to_markdown: bool
    output_directory: Path | None = None
    improvement_mode: ImprovementMode | None = None
    target_language: str | None = None
    offline_translation_language: str | None = None
    pdf_page_range: PdfPageRange | None = None
    force_pdf_ocr: bool = False
    output_format: OutputFormat = OutputFormat.MARKDOWN
    image_output_directory: Path | None = None
    epub_title: str | None = None
    epub_author: str | None = None
    epub_cover_path: Path | None = None
    glossary: tuple[GlossaryEntry, ...] = ()
    include_images: bool = True
    preserve_styles: bool = True
    markdown_organization: MarkdownOrganization = MarkdownOrganization.SINGLE_FILE
    markdown_include_metadata: bool = False
    markdown_include_page_references: bool = False
    epub_first_page_cover: bool = False
    epub_remove_cover: bool = False
    review_content: bool = False
    review_structure: bool = False
    source_size_bytes: int | None = None
    source_modified_ns: int | None = None
    source_content_sha256: str | None = None
    source_identity_verified: bool = False
    execution_plan: ExecutionPlan | None = None

    @property
    def source(self) -> ProcessSourceRequest:
        """Source view consumed by batch validation."""

        return ProcessSourceRequest(
            self.source_path,
            self.convert_to_markdown,
            self.pdf_page_range,
            self.force_pdf_ocr,
            self.source_size_bytes,
            self.source_modified_ns,
            self.source_content_sha256,
            self.source_identity_verified,
        )

    @property
    def publication(self) -> ProcessPublicationRequest:
        """Publication view consumed by batch validation."""

        return ProcessPublicationRequest(
            self.output_directory,
            self.output_format,
            self.image_output_directory,
            self.epub_title,
            self.epub_author,
            self.epub_cover_path,
            self.include_images,
            self.preserve_styles,
            self.markdown_organization,
            self.markdown_include_metadata,
            self.markdown_include_page_references,
            self.epub_first_page_cover,
            self.epub_remove_cover,
        )


@dataclass(frozen=True, slots=True)
class ProcessResult:
    """Stable facade result returned to application and presentation layers."""

    final_path: Path
    raw_markdown_path: Path | None = None
    review_original_path: Path | None = None
    problematic_pdf_pages: tuple[int, ...] = ()
    pdf_quality_report: PdfQualityReport | None = None
    exhaustive_pdf_ocr_used: bool = False
    epub_translation_parts: int = 0
    epub_resumed_parts: int = 0
    epub_checkpoint_degraded: bool = False
    translation_quality_report: TranslationQualityReport | None = None
    review_translation_quality_report: TranslationQualityReport | None = None
    linguistic_review_coverage: LinguisticReviewCoverage | None = None
    preserved_translation_chunks: tuple[int, ...] = ()
    preserved_review_chunks: int = 0
    preserved_images: int = 0
    epub_chapters: int = 0
    revision_draft: RevisionDraft | None = None
    revision_resources: tuple[ConvertedResource, ...] = ()
    revision_epub_metadata: EpubBookMetadata | None = None
    review_markdown: str | None = None
    review_required: bool = False
    revision_approved: bool = False
    preserve_epub_package_on_unchanged_review: bool = False
    final_integrity_report: FinalIntegrityReport | None = None
    telemetry: ProcessTelemetry | None = None
    front_matter_blocks: int = 0
    toc_blocks: int = 0
    terminology_terms: int = 0
    markdown_organization: MarkdownOrganization = MarkdownOrganization.SINGLE_FILE
    markdown_include_metadata: bool = False
    markdown_include_page_references: bool = False
    markdown_source_name: str | None = None

    @property
    def translation_quality_for_review(self) -> TranslationQualityReport | None:
        """Return quality evidence aligned with ``review_markdown`` when available."""

        return self.review_translation_quality_report or self.translation_quality_report


@dataclass(frozen=True, slots=True)
class PreparedDocument:
    """In-memory output of source preparation; it has not been published."""

    source_path: Path
    resolved_page_range: PdfPageRange | None
    output_stem: str | None
    pdf_quality_report: PdfQualityReport | None
    source_cover_path: PurePosixPath | None
    converted_resources: tuple[ConvertedResource, ...]
    markdown: str
    problematic_pdf_pages: tuple[int, ...]
    semantic_document: SemanticDocument
    translation_glossary: tuple[GlossaryEntry, ...]


@dataclass(frozen=True, slots=True)
class TransformedDocument:
    """In-memory output of transformations; it has not been published."""

    transformed_markdown: str
    translation_quality_report: TranslationQualityReport | None
    review_translation_quality_report: TranslationQualityReport | None
    linguistic_review_coverage: LinguisticReviewCoverage | None
    preserved_translation_chunks: tuple[int, ...]
    revision_draft: RevisionDraft | None
    published_markdown: str
    review_required: bool
    public_markdown: str
    preserved_review_chunks: int = 0


StageCallback = Callable[[ProcessStage], None]
ProgressCallback = Callable[[int, int], None]


__all__ = [
    "PreparedDocument",
    "ProcessPublicationRequest",
    "ProcessRequest",
    "ProcessResult",
    "ProcessSourceRequest",
    "ProcessTelemetry",
    "ProgressCallback",
    "StageCallback",
    "StageTelemetry",
    "TransformedDocument",
]
