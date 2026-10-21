"""Mide la lista de revisión (F4) con los errores reales de las páginas de referencia.

En los escaneos con páginas transcritas a mano (`eval/references/<id>/page-NNNN.md`), un fragmento
del resultado es erróneo si tiene alguna palabra que la transcripción de su página no contiene. Se
mide qué parte de esos fragmentos cae en el primer 10, 20 y 50 % de la lista (revisar al azar
encontraría el 10, 20 y 50 %). Solo escribe recuentos; nunca contenido documental.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tempfile
from pathlib import Path

import pypdfium2 as pdfium

from eval_review import content_blocks


def words(text: str) -> list[str]:
    return [w.lower() for w in re.findall(r"\w+", text) if len(w) >= 2]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("eval/manifest.json"))
    parser.add_argument("--references", type=Path, default=Path("eval/references"))
    parser.add_argument("--pdf-dir", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--tool", type=Path, required=True)
    parser.add_argument("--pdfium", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    results = []
    for document in manifest["documents"]:
        pages = document.get("reference_pages") or []
        source = args.candidate / document["id"] / "document.md"
        if not pages or not source.exists():
            continue
        markdown = source.read_text(encoding="utf-8").replace("\r\n", "\n")
        first = (document.get("pages") or [1])[0]
        with tempfile.TemporaryDirectory() as temporary:
            pdf = args.pdf_dir / f"{document['id']}.pdf"
            if document.get("pages"):
                original = pdfium.PdfDocument(str(pdf))
                subset = pdfium.PdfDocument.new()
                last = document["pages"][1]
                subset.import_pages(original, list(range(first - 1, min(last, len(original)))))
                pdf = Path(temporary) / "subset.pdf"
                subset.save(str(pdf))
            items = json.loads(
                subprocess.run(
                    [str(args.tool), str(args.pdfium), str(pdf), str(source)],
                    capture_output=True,
                    check=True,
                ).stdout
            )
        order = [(i["page"], i["block"]) for i in items if i["block"] is not None]
        blocks = content_blocks(markdown)
        erroneous = set()
        checked = 0
        for number in pages:
            reference = set(
                words(
                    (args.references / document["id"] / f"page-{number:04d}.md").read_text(
                        encoding="utf-8"
                    )
                )
            )
            relative = number - first + 1
            for page, index, start, end in blocks:
                if page != relative:
                    continue
                checked += 1
                if any(w not in reference for w in words(markdown[start:end])):
                    erroneous.add((page, index))
        total = len(blocks)
        found = {}
        for share in (0.1, 0.2, 0.5):
            top = set(order[: max(1, round(total * share))])
            found[share] = len(erroneous & top)
        results.append(
            {
                "id": document["id"],
                "blocks": total,
                "reference_blocks": checked,
                "erroneous": len(erroneous),
                "flagged": len(erroneous & set(order)),
                "found_top10": found[0.1],
                "found_top20": found[0.2],
                "found_top50": found[0.5],
            }
        )
        print(
            f"{document['id'][:32]:32} fragmentos de referencia {checked:3}, con errores "
            f"{len(erroneous):3}, avisados {len(erroneous & set(order)):3}: "
            f"10 % -> {found[0.1]:2}, 20 % -> {found[0.2]:2}, 50 % -> {found[0.5]:2}",
            flush=True,
        )
    total_errors = sum(r["erroneous"] for r in results)
    summary = {
        share: sum(r[f"found_top{int(share * 100)}"] for r in results) / max(1, total_errors)
        for share in (0.1, 0.2, 0.5)
    }
    flagged = sum(r["flagged"] for r in results) / max(1, total_errors)
    print(
        "total:",
        {f"{int(k * 100)} %": round(v * 100, 1) for k, v in summary.items()},
        "avisados:",
        round(flagged * 100, 1),
    )
    args.out.write_text(
        json.dumps({"documents": results, "summary": summary, "flagged": flagged}, indent=1),
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
