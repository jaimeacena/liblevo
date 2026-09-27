"""One short synthetic PDF-to-Spanish-EPUB run through the real local model."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from time import perf_counter
from xml.etree import ElementTree
from zipfile import ZipFile

REPOSITORY = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY))
sys.path.insert(0, str(REPOSITORY / "src"))

from scripts.validate_real_workflows import _write_text_pdf  # noqa: E402

from parsezen.component_catalog import (  # noqa: E402
    TRANSLATION_CONTEXT_WINDOW,
    TRANSLATION_MODEL_NAME,
)
from parsezen.improvement import ImprovementMode  # noqa: E402
from parsezen.processing import OutputFormat, ProcessRequest, process_document  # noqa: E402
from parsezen.settings import AppSettings  # noqa: E402


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    root = Path(__file__).resolve().parent
    source = root / "synthetic-translation-2-pages.pdf"
    output = root / "translated-output"
    if source.exists() or output.exists():
        raise RuntimeError("Audit translation artifacts already exist; refusing to overwrite")

    pages = (
        (
            "Chapter One",
            (
                "The traveler did not leave on Monday.",
                "She carried 12 blue notebooks and three maps.",
                "The bridge was closed, so the group waited until Tuesday.",
                "Only Ana received the letter; Bruno did not.",
            ),
        ),
        (
            "Chapter Two",
            (
                "At 7:30, the guide counted 48 people, not 84.",
                "Every visitor must return before sunset.",
                "The second room was quieter than the first.",
                "The river bank was steep after the rain.",
            ),
        ),
    )
    _write_text_pdf(source, pages)
    output.mkdir()
    source_before = digest(source)
    start = perf_counter()
    result = process_document(
        ProcessRequest(
            source,
            convert_to_markdown=True,
            output_directory=output,
            output_format=OutputFormat.EPUB,
            improvement_mode=ImprovementMode.TRANSLATE,
            target_language="Español",
            epub_title="Prueba sintética de traducción",
            epub_author="Auditoría",
        ),
        settings=AppSettings(
            model=TRANSLATION_MODEL_NAME,
            context_window=TRANSLATION_CONTEXT_WINDOW,
            timeout_seconds=300.0,
        ),
        work_checkpoint_root=root / "translation-checkpoints",
        epub_checkpoint_root=root / "translation-epub-checkpoints",
    )
    elapsed = round(perf_counter() - start, 3)

    with ZipFile(result.final_path) as archive:
        corrupt_member = archive.testzip()
        names = archive.namelist()
        text = " ".join(
            " ".join(ElementTree.fromstring(archive.read(name)).itertext())
            for name in names
            if name.endswith(".xhtml")
        )
    normalized = re.sub(r"\s+", " ", text)
    expected_literals = ("12", "7:30", "48", "84", "Ana", "Bruno")
    missing_literals = [
        value
        for value in expected_literals
        if re.search(rf"\b{re.escape(value)}\b", normalized) is None
    ]
    source_unchanged = digest(source) == source_before
    report = {
        "case": "SYN-TRANSLATE-EPUB-2",
        "result": (
            "PASS"
            if source_unchanged and corrupt_member is None and not missing_literals
            else "FAIL"
        ),
        "elapsed_seconds": elapsed,
        "input_pages": 2,
        "input_sha256_unchanged": source_unchanged,
        "output_bytes": result.final_path.stat().st_size,
        "archive_corrupt_member": corrupt_member,
        "missing_numbers_or_names": missing_literals,
        "reported_chapters": result.epub_chapters,
        "preserved_translation_chunks": len(result.preserved_translation_chunks),
        "reported_translation_issues": (
            result.translation_quality_report.total_issues
            if result.translation_quality_report is not None
            else None
        ),
        "semantic_fidelity_human_reviewed": False,
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
