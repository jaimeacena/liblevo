from pathlib import Path

from liblevo.domain.jobs import MarkdownOrganization
from liblevo.improvement import ImprovementMode
from liblevo.pipeline.contracts import (
    ProcessRequest as PipelineRequest,
)
from liblevo.pipeline.contracts import (
    ProcessResult as PipelineResult,
)
from liblevo.pipeline.contracts import (
    ProcessTelemetry as PipelineTelemetry,
)
from liblevo.processing import ProcessRequest, ProcessResult, ProcessTelemetry
from liblevo.workflow import OutputFormat


def test_processing_facade_reexports_the_pipeline_contracts() -> None:
    assert ProcessRequest is PipelineRequest
    assert ProcessResult is PipelineResult
    assert ProcessTelemetry is PipelineTelemetry


def test_process_contracts_keep_the_views_used_by_batch_validation() -> None:
    request = ProcessRequest(
        Path("source.pdf"),
        True,
        output_directory=Path("output"),
        improvement_mode=ImprovementMode.TRANSLATE,
        target_language="es",
        output_format=OutputFormat.EPUB,
        review_content=True,
        markdown_organization=MarkdownOrganization.BY_CHAPTER,
        source_content_sha256="a" * 64,
    )
    result = ProcessResult(
        Path("output/book.epub"),
        review_markdown="local review text",
        review_required=True,
        problematic_pdf_pages=(4,),
    )

    assert request.source.path == request.source_path
    assert request.source.content_sha256 == "a" * 64
    assert request.target_language == "es"
    assert request.review_content
    assert request.publication.output_format is OutputFormat.EPUB
    assert result.review_markdown == "local review text"
    assert result.final_path == Path("output/book.epub")
    assert result.problematic_pdf_pages == (4,)
