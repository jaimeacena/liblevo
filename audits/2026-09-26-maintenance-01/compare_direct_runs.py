"""Compare two synthetic EPUBs without recording their text."""

from __future__ import annotations

import hashlib
import json
import re
from difflib import SequenceMatcher
from pathlib import Path
from xml.etree import ElementTree
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parent


def _read_epub(path: Path) -> tuple[dict[str, bytes], str]:
    with ZipFile(path) as archive:
        members = {name: archive.read(name) for name in archive.namelist()}
    text = " ".join(
        " ".join(ElementTree.fromstring(payload).itertext())
        for name, payload in sorted(members.items())
        if name.endswith(".xhtml")
    )
    return members, re.sub(r"\s+", " ", text).strip()


def main() -> None:
    destination = ROOT / "direct-20-comparison.json"
    if destination.exists():
        raise FileExistsError("Refusing to overwrite an earlier comparison")
    paths = [next((ROOT / f"direct-20-output-{run}").glob("*.epub")) for run in (1, 2)]
    first, first_text = _read_epub(paths[0])
    second, second_text = _read_epub(paths[1])
    changed = sorted(name for name in first.keys() & second.keys() if first[name] != second[name])
    result = {
        "identical_epub_bytes": paths[0].read_bytes() == paths[1].read_bytes(),
        "identical_member_names": first.keys() == second.keys(),
        "member_count": len(first),
        "different_member_count": len(changed),
        "different_xhtml_member_count": sum(name.endswith(".xhtml") for name in changed),
        "identical_normalized_reading_text": first_text == second_text,
        "normalized_text_sha256": [
            hashlib.sha256(value.encode("utf-8")).hexdigest() for value in (first_text, second_text)
        ],
        "normalized_text_characters": [len(first_text), len(second_text)],
        "text_similarity_ratio": round(
            SequenceMatcher(a=first_text, b=second_text, autojunk=False).ratio(), 6
        ),
    }
    with destination.open("x", encoding="utf-8") as stream:
        stream.write(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()
