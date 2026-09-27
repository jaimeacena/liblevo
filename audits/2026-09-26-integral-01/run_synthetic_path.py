"""Isolated, synthetic PDF-to-EPUB audit; never reads a personal document."""

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

from scripts.validate_real_workflows import write_synthetic_pdf  # noqa: E402

from parsezen.processing import OutputFormat, ProcessRequest, process_document  # noqa: E402


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    root = Path(__file__).resolve().parent
    source = root / "synthetic-20-pages.pdf"
    output = root / "synthetic-output"
    if output.exists():
        raise RuntimeError("Audit output already exists; refusing to overwrite")
    if not source.exists():
        write_synthetic_pdf(source)
    output.mkdir()
    source_before = digest(source)
    start = perf_counter()
    result = process_document(
        ProcessRequest(
            source,
            convert_to_markdown=True,
            output_directory=output,
            output_format=OutputFormat.EPUB,
            epub_title="Synthetic audit book",
            epub_author="Audit sample",
        ),
        work_checkpoint_root=root / "synthetic-checkpoints",
        epub_checkpoint_root=root / "synthetic-epub-checkpoints",
    )
    elapsed = round(perf_counter() - start, 3)
    source_after = digest(source)

    with ZipFile(result.final_path) as archive:
        corrupt_member = archive.testzip()
        names = archive.namelist()
        xhtml = [name for name in names if name.endswith(".xhtml")]
        text = " ".join(
            " ".join(ElementTree.fromstring(archive.read(name)).itertext()) for name in xhtml
        )
        compressed = re.sub(r"\s+", " ", text)

    missing_values = [
        page * 17 for page in range(3, 21) if re.search(rf"\b{page * 17}\b", compressed) is None
    ]
    expected_headings = ("PRACTICAL GUIDE", "FINAL CHECKS", "PUBLICATION REVIEW")
    missing_headings = [heading for heading in expected_headings if heading not in compressed]
    report = {
        "case": "SYN-PDF-EPUB-20",
        "result": (
            "PASS"
            if source_before == source_after
            and corrupt_member is None
            and not missing_values
            and not missing_headings
            else "FAIL"
        ),
        "elapsed_seconds": elapsed,
        "input_pages": 20,
        "input_sha256_unchanged": source_before == source_after,
        "output_bytes": result.final_path.stat().st_size,
        "archive_corrupt_member": corrupt_member,
        "xhtml_members": len(xhtml),
        "has_navigation": any(name.endswith("nav.xhtml") for name in names),
        "missing_reference_values": missing_values,
        "missing_headings": missing_headings,
        "reported_chapters": result.epub_chapters,
        "translation_tested": False,
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
