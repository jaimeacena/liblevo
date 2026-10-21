"""Mide la lista de revisión (F4) con errores plantados en los resultados del conjunto.

Para cada documento planta errores de tipos conocidos en fragmentos al azar (semilla fija), calcula
la lista con `liblevo-revisar` y mide qué parte de los errores cae en el primer 10, 20 y 50 % de los
fragmentos revisados en el orden de la lista. Revisar al azar encontraría el 10, 20 y 50 %. Los
avisos reales del documento compiten con los plantados, como en el uso normal. Solo escribe
recuentos; nunca contenido documental.
"""

from __future__ import annotations

import argparse
import json
import random
import re
import subprocess
import tempfile
from pathlib import Path

import pypdfium2 as pdfium

MARKER = re.compile(r"<!-- page: (\d+) -->")
CONFUSIONS = [("e", "c"), ("l", "1"), ("o", "0"), ("m", "rn"), ("h", "b"), ("i", "í"), ("a", "o")]
KINDS = ["confusión", "partida", "pegada", "símbolo", "borrada"]


def content_blocks(markdown: str) -> list[tuple[int, int, int, int]]:
    """(página, índice en la página, inicio, fin) de cada fragmento de contenido."""
    blocks: list[tuple[int, int, int, int]] = []
    page, index, position = 0, 0, 0
    for raw in markdown.split("\n\n"):
        start = position
        position += len(raw) + 2
        text = raw.strip()
        found = MARKER.search(text)
        if found and text.startswith("<!--"):
            page, index = int(found.group(1)), 0
            continue
        if not text or text.startswith("![") or text.startswith("<!--") or page == 0:
            continue
        offset = raw.find(text)
        blocks.append((page, index, start + offset, start + offset + len(text)))
        index += 1
    return blocks


def plant(text: str, kind: str, rng: random.Random) -> str | None:
    words = [m for m in re.finditer(r"[^\W\d_]{4,}", text)]
    if not words:
        return None
    word = rng.choice(words)
    w, s, e = word.group(0), word.start(), word.end()
    if kind == "confusión":
        options = [(a, b) for a, b in CONFUSIONS if a in w]
        if not options:
            return None
        a, b = rng.choice(options)
        k = w.index(a)
        new = w[:k] + b + w[k + len(a) :]
    elif kind == "partida":
        k = rng.randint(1, len(w) - 1)
        new = w[:k] + " " + w[k:]
    elif kind == "pegada":
        rest = text[e:]
        nxt = re.match(r"\s+([^\W\d_]+)", rest)
        if not nxt:
            return None
        return text[:s] + w + nxt.group(1) + rest[nxt.end() :]
    elif kind == "símbolo":
        new = w + " ¤"
    else:  # borrada
        return text[:s] + text[e:].lstrip()
    return text[:s] + new + text[e:]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("eval/manifest.json"))
    parser.add_argument("--pdf-dir", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--tool", type=Path, required=True)
    parser.add_argument("--pdfium", type=Path, required=True)
    parser.add_argument("--per-document", type=int, default=15)
    parser.add_argument("--seed", type=int, default=20261019)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    results = []
    for document in manifest["documents"]:
        source = args.candidate / document["id"] / "document.md"
        if not source.exists():
            continue
        rng = random.Random(f"{args.seed}-{document['id']}")
        markdown = source.read_text(encoding="utf-8").replace("\r\n", "\n")
        blocks = content_blocks(markdown)
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            pdf = args.pdf_dir / f"{document['id']}.pdf"
            if document.get("pages"):
                first, last = document["pages"]
                original = pdfium.PdfDocument(str(pdf))
                subset = pdfium.PdfDocument.new()
                subset.import_pages(original, list(range(first - 1, min(last, len(original)))))
                pdf = folder / "subset.pdf"
                subset.save(str(pdf))
            planted: list[tuple[int, int, str]] = []
            chosen = rng.sample(range(len(blocks)), min(len(blocks), args.per_document * 3))
            edits: dict[int, str] = {}
            for block_index in chosen:
                if len(planted) >= args.per_document:
                    break
                page, index, start, end = blocks[block_index]
                kind = KINDS[len(planted) % len(KINDS)]
                changed = plant(markdown[start:end], kind, rng)
                if changed is None or "\n\n" in changed:
                    continue
                edits[block_index] = changed
                planted.append((page, index, kind))
            pieces, cursor = [], 0
            for block_index in sorted(edits):
                _, _, start, end = blocks[block_index]
                pieces += [markdown[cursor:start], edits[block_index]]
                cursor = end
            pieces.append(markdown[cursor:])
            target = folder / "document.md"
            target.write_text("".join(pieces), encoding="utf-8")
            output = subprocess.run(
                [str(args.tool), str(args.pdfium), str(pdf), str(target)],
                capture_output=True,
                check=True,
            ).stdout
        items = json.loads(output)
        order = [(i["page"], i["block"]) for i in items if i["block"] is not None]
        total = len(blocks)
        found = {}
        for share in (0.1, 0.2, 0.5):
            top = set(order[: max(1, round(total * share))])
            found[share] = sum(1 for p, b, _ in planted if (p, b) in top)
        flagged = set(order)
        by_kind = {
            k: [sum(1 for p, b, kk in planted if kk == k and (p, b) in flagged),
                sum(1 for _, _, kk in planted if kk == k)]
            for k in KINDS
        }  # fmt: skip
        results.append(
            {
                "id": document["id"],
                "blocks": total,
                "flagged_blocks": len(order),
                "planted": len(planted),
                "found_top10": found[0.1],
                "found_top20": found[0.2],
                "found_top50": found[0.5],
                "flagged_by_kind": by_kind,
            }
        )
        print(
            f"{document['id'][:32]:32} fragmentos {total:5} avisados {len(order):4} "
            f"plantados {len(planted):2}: 10 % -> {found[0.1]:2}, 20 % -> {found[0.2]:2}, "
            f"50 % -> {found[0.5]:2}",
            flush=True,
        )
    planted_total = sum(r["planted"] for r in results)
    summary = {
        share: sum(r[f"found_top{int(share * 100)}"] for r in results) / max(1, planted_total)
        for share in (0.1, 0.2, 0.5)
    }
    kinds_total = {
        k: [sum(r["flagged_by_kind"][k][0] for r in results),
            sum(r["flagged_by_kind"][k][1] for r in results)]
        for k in KINDS
    }  # fmt: skip
    print("total:", {f"{int(k * 100)} %": round(v * 100, 1) for k, v in summary.items()})
    print("avisados por tipo:", kinds_total)
    args.out.write_text(
        json.dumps({"documents": results, "summary": summary, "by_kind": kinds_total}, indent=1),
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
