"""Compare visible text of two synthetic EPUB results without logging content."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from xml.etree import ElementTree
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parent
OLLAMA_EPUB = (
    ROOT.parent
    / "2026-09-26-integral-01"
    / "translated-output"
    / "synthetic-translation-2-pages.es.epub"
)
DIRECT_EPUB = ROOT / "direct-epub-output" / "synthetic-translation-2-pages.es.epub"
EVIDENCE = ROOT / "synthetic-epub-body-comparison.json"


def visible_text(path: Path) -> str:
    with ZipFile(path) as archive:
        members = sorted(
            name
            for name in archive.namelist()
            if name.endswith(".xhtml") and not name.endswith("/nav.xhtml")
        )
        paragraphs = [
            " ".join(ElementTree.fromstring(archive.read(name)).itertext()) for name in members
        ]
    return re.sub(r"\s+", " ", " ".join(paragraphs)).strip()


def main() -> None:
    if EVIDENCE.exists():
        raise RuntimeError("Comparison evidence already exists")
    ollama_text = visible_text(OLLAMA_EPUB)
    direct_text = visible_text(DIRECT_EPUB)
    report = {
        "case": "DIRECT-OLLAMA-SYNTHETIC-BODY-01",
        "same_visible_text": ollama_text == direct_text,
        "ollama_text_sha256": hashlib.sha256(ollama_text.encode("utf-8")).hexdigest(),
        "direct_text_sha256": hashlib.sha256(direct_text.encode("utf-8")).hexdigest(),
        "ollama_characters": len(ollama_text),
        "direct_characters": len(direct_text),
        "document_text_logged": False,
    }
    EVIDENCE.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
