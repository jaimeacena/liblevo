"""Content-free structural inventory for the 2026-09-26 maintenance audit."""

from __future__ import annotations

import ast
import json
from collections import Counter
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[2]
SOURCES = PROJECT / "src" / "parsezen"


def module_name(path: Path) -> str:
    parts = path.relative_to(SOURCES).with_suffix("").parts
    return "parsezen" + ("." + ".".join(parts) if parts else "")


def main() -> None:
    records: list[dict[str, object]] = []
    imports: dict[str, set[str]] = {}
    symbols: list[dict[str, object]] = []
    for path in sorted(SOURCES.rglob("*.py")):
        source = path.read_text(encoding="utf-8")
        tree = ast.parse(source, filename=str(path))
        name = module_name(path)
        direct_imports: set[str] = set()
        for node in ast.walk(tree):
            if (
                isinstance(node, ast.ImportFrom)
                and node.module
                and node.module.startswith("parsezen")
            ):
                direct_imports.add(node.module)
            elif isinstance(node, ast.Import):
                direct_imports.update(
                    alias.name for alias in node.names if alias.name.startswith("parsezen")
                )
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                symbols.append(
                    {
                        "module": name,
                        "name": node.name,
                        "kind": type(node).__name__,
                        "lines": (node.end_lineno or node.lineno) - node.lineno + 1,
                    }
                )
        imports[name] = direct_imports
        records.append(
            {
                "module": name,
                "path": str(path.relative_to(PROJECT)).replace("\\", "/"),
                "lines": len(source.splitlines()),
                "functions": sum(
                    isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                    for node in ast.walk(tree)
                ),
                "classes": sum(isinstance(node, ast.ClassDef) for node in ast.walk(tree)),
                "branches": sum(
                    isinstance(node, (ast.If, ast.For, ast.While, ast.Try, ast.Match))
                    for node in ast.walk(tree)
                ),
                "broad_excepts": sum(
                    isinstance(node, ast.ExceptHandler)
                    and isinstance(node.type, ast.Name)
                    and node.type.id in {"Exception", "BaseException"}
                    for node in ast.walk(tree)
                ),
            }
        )
    incoming = Counter(target for values in imports.values() for target in values)
    for record in records:
        record["internal_imports"] = len(imports[record["module"]])
        record["incoming_imports"] = incoming[record["module"]]
    tests = sorted((PROJECT / "tests").glob("test_*.py"))
    test_functions = 0
    for path in tests:
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        test_functions += sum(
            isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and node.name.startswith("test_")
            for node in ast.walk(tree)
        )
    docs = [
        {
            "path": str(path.relative_to(PROJECT)).replace("\\", "/"),
            "lines": len(path.read_text(encoding="utf-8").splitlines()),
        }
        for path in (PROJECT / "docs").glob("*.md")
    ]
    result = {
        "source_module_count": len(records),
        "source_lines": sum(int(record["lines"]) for record in records),
        "test_file_count": len(tests),
        "test_function_count": test_functions,
        "top_modules_by_lines": sorted(records, key=lambda item: item["lines"], reverse=True)[:25],
        "top_modules_by_incoming_imports": sorted(
            records, key=lambda item: item["incoming_imports"], reverse=True
        )[:20],
        "top_symbols_by_lines": sorted(symbols, key=lambda item: item["lines"], reverse=True)[:20],
        "docs_by_lines": sorted(docs, key=lambda item: item["lines"], reverse=True),
    }
    destination = Path(__file__).with_name("structure.json")
    with destination.open("x", encoding="utf-8") as stream:
        stream.write(json.dumps(result, ensure_ascii=False, indent=2))
    print(json.dumps({key: result[key] for key in list(result)[:4]}, indent=2))
    for record in result["top_modules_by_lines"][:10]:
        print(record["path"], record["lines"], "branches", record["branches"])


if __name__ == "__main__":
    main()
