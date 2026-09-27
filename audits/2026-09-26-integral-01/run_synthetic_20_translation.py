"""Translate the already-generated synthetic 20-page PDF using local Ollama only."""

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
sys.path.insert(0, str(REPOSITORY / "src"))

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
    source = root / "synthetic-20-pages.pdf"
    output = root / "translated-20-output"
    evidence = root / "translated-20-evidence.json"
    if not source.is_file() or output.exists() or evidence.exists():
        raise RuntimeError("Synthetic source is absent or audit output already exists")
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
            epub_title="Libro sintético traducido",
            epub_author="Auditoría",
        ),
        settings=AppSettings(
            model=TRANSLATION_MODEL_NAME,
            context_window=TRANSLATION_CONTEXT_WINDOW,
            timeout_seconds=300.0,
        ),
        work_checkpoint_root=root / "translated-20-work-checkpoints",
        epub_checkpoint_root=root / "translated-20-epub-checkpoints",
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
    missing_values = [
        page * 17 for page in range(3, 21) if re.search(rf"\b{page * 17}\b", normalized) is None
    ]
    source_unchanged = digest(source) == source_before
    passed = source_unchanged and corrupt_member is None and not missing_values
    report = {
        "case": "SYN-TR-20",
        "result": "PASS" if passed else "FAIL",
        "elapsed_seconds": elapsed,
        "input_pages": 20,
        "input_sha256_unchanged": source_unchanged,
        "output_bytes": result.final_path.stat().st_size,
        "archive_corrupt_member": corrupt_member,
        "has_navigation": any(name.endswith("nav.xhtml") for name in names),
        "missing_reference_values": missing_values,
        "reported_chapters": result.epub_chapters,
        "reported_translation_issues": (
            result.translation_quality_report.total_issues
            if result.translation_quality_report is not None
            else None
        ),
        "semantic_fidelity_human_reviewed": False,
    }
    evidence.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
