"""Small local A/B evaluation: frozen inputs, real runs, blinded human review.

Private files contain documents. Only ``report`` emits shareable aggregate data.
Existing corpora are never discovered or imported. No semantic auto-approval.
"""

from __future__ import annotations

import argparse
import hashlib
import html
import importlib.metadata
import json
import logging
import math
import os
import platform
import random
import secrets
import shutil
import sys
import threading
import time
from collections import Counter
from collections.abc import Iterator
from contextlib import contextmanager
from datetime import UTC, datetime
from pathlib import Path
from statistics import median
from typing import Any
from uuid import UUID, uuid4

ROOT = Path(__file__).resolve().parents[1]
VERSION = 1
MAX_JSON = 8 * 1024 * 1024
MAX_SOURCE = 512 * 1024 * 1024
LABELS = {"no_error_observed", "minor", "major", "not_evaluable"}
COHORTS = {"representative", "risk", "known"}
FORMATS = {".txt", ".md", ".pdf", ".epub", ".docx"}


class EvaluationError(ValueError):
    """Safe diagnosis; never attach paths, document values or server responses."""


def digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=True).encode()).hexdigest()


def file_hash(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def read_json(path: Path) -> Any:
    if path.stat().st_size > MAX_JSON:
        raise EvaluationError("El archivo de evaluación supera el límite permitido.")
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    """Exclusive creation: historical evidence is never overwritten."""
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2, allow_nan=False)
        stream.flush()
        os.fsync(stream.fileno())


def utc_now() -> str:
    return datetime.now(UTC).isoformat()


def write_evidence(path: Path, value: Any) -> None:
    write_json(path, value)
    with path.with_suffix(".sha256").open("x", encoding="ascii") as stream:
        stream.write(file_hash(path))


def read_evidence(path: Path) -> Any:
    if file_hash(path) != path.with_suffix(".sha256").read_text(encoding="ascii"):
        raise EvaluationError("La evidencia guardada fue modificada o está incompleta.")
    return read_json(path)


def evidence_complete(path: Path) -> bool:
    """A process may stop between writing a record and its integrity receipt."""
    return path.is_file() and path.with_suffix(".sha256").is_file()


def _uuid(value: Any) -> str:
    if not isinstance(value, str) or str(UUID(value)) != value:
        raise EvaluationError("Se necesita un identificador UUID canónico.")
    return value


def _sha(value: Any) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(c not in "0123456789abcdef" for c in value)
    ):
        raise EvaluationError("La huella no es SHA-256 válido.")
    return value


def _local(path: Path) -> Path:
    if str(path).startswith(("\\\\", "//")):
        raise EvaluationError("El origen debe ser local.")
    resolved = path.resolve(strict=True)
    if str(resolved).startswith(("\\\\", "//")) or not resolved.is_file():
        raise EvaluationError("El origen debe ser un archivo local.")
    if resolved.suffix.lower() not in FORMATS or resolved.stat().st_size > MAX_SOURCE:
        raise EvaluationError("Formato o tamaño de origen no admitido.")
    return resolved


def _options(raw: Any) -> dict[str, Any]:
    if not isinstance(raw, dict) or set(raw) - {
        "output_format",
        "translate_to",
        "force_ocr",
        "include_images",
        "pages",
    }:
        raise EvaluationError("Opciones de evaluación desconocidas.")
    result = {
        "output_format": raw.get("output_format", "markdown"),
        "translate_to": raw.get("translate_to"),
        "force_ocr": raw.get("force_ocr", False),
        "include_images": raw.get("include_images", True),
        "pages": raw.get("pages"),
    }
    if result["output_format"] not in {"markdown", "epub"}:
        raise EvaluationError("Formato de salida desconocido.")
    language = result["translate_to"]
    if language is not None and (not isinstance(language, str) or language not in {"es", "en"}):
        raise EvaluationError("Este evaluador inicial admite traducción a es o en.")
    if any(type(result[key]) is not bool for key in ("force_ocr", "include_images")):
        raise EvaluationError("Las opciones booleanas deben ser explícitas.")
    pages = result["pages"]
    if pages is not None and (
        not isinstance(pages, list)
        or len(pages) != 2
        or any(type(number) is not int for number in pages)
        or not 1 <= pages[0] <= pages[1]
    ):
        raise EvaluationError("Intervalo de páginas no válido.")
    return result


def load_catalog(path: Path) -> dict[str, Any]:
    raw = read_json(path)
    if not isinstance(raw, dict) or raw.get("version") != VERSION:
        raise EvaluationError("Versión de catálogo no admitida.")
    cases = raw.get("cases")
    if not isinstance(cases, list) or not 1 <= len(cases) <= 500:
        raise EvaluationError("Añada entre uno y 500 casos al catálogo vacío.")
    ids: set[str] = set()
    for case in cases:
        if not isinstance(case, dict):
            raise EvaluationError("Caso no válido.")
        identifier = _uuid(case.get("id"))
        if identifier in ids:
            raise EvaluationError("Hay identificadores de caso duplicados.")
        ids.add(identifier)
        source = _local(path.parent / case["source"])
        if file_hash(source) != _sha(case.get("source_sha256")):
            raise EvaluationError("Un original no coincide con su huella declarada.")
        if case.get("cohort") not in COHORTS:
            raise EvaluationError("Declare grupo representative, risk o known.")
        if case.get("provenance", "unverified") not in {"unverified", "new", "known", "synthetic"}:
            raise EvaluationError("Procedencia no válida.")
        case["provenance"] = case.get("provenance", "unverified")
        case["source"] = str(source)
        case["options"] = _options(case.get("options", {}))
        if case["options"]["pages"] and source.suffix.lower() != ".pdf":
            raise EvaluationError("Los intervalos de páginas solo se aplican a PDF.")
        reference = case.setdefault("reference", {"status": "unverified", "contains": []})
        if not isinstance(reference, dict) or reference.get("status") not in {
            "unverified",
            "human_verified",
            "synthetic",
        }:
            raise EvaluationError("Estado de referencia no válido.")
        values = reference.get("contains", [])
        if (
            not isinstance(values, list)
            or len(values) > 50
            or any(not isinstance(item, str) or not 1 <= len(item) <= 2000 for item in values)
        ):
            raise EvaluationError("Las expectativas literales deben ser pequeñas y explícitas.")
        if reference["status"] == "human_verified" and (
            reference.get("source_sha256") != case["source_sha256"]
            or reference.get("expectations_sha256") != digest(values)
            or not reference.get("reviewed_at")
        ):
            raise EvaluationError("La referencia humana no acredita estas expectativas y original.")
        reference["contains"] = values
    return raw


def freeze(
    catalog: Path,
    destination: Path,
    *,
    representative: int,
    risk: int,
    known: int,
    seed: int,
    fresh: bool,
    repetitions: int,
) -> dict[str, Any]:
    raw = load_catalog(catalog)
    if type(repetitions) is not int or not 1 <= repetitions <= 3:
        raise EvaluationError("Declare de una a tres repeticiones antes de ejecutar.")
    if not isinstance(raw.get("question"), str) or not raw["question"].strip():
        raise EvaluationError("Declare qué pregunta resolverá esta comparación.")
    if not isinstance(raw.get("acceptance"), str) or not raw["acceptance"].strip():
        raise EvaluationError("Declare la condición de aceptación antes de ejecutar.")
    selected = []
    rng = random.Random(seed)
    for cohort, count in (("representative", representative), ("risk", risk), ("known", known)):
        candidates = sorted(
            (c for c in raw["cases"] if c["cohort"] == cohort), key=lambda c: c["id"]
        )
        if type(count) is not int or not 0 <= count <= len(candidates):
            raise EvaluationError("La muestra solicitada supera los casos disponibles.")
        selected.extend(rng.sample(candidates, count))
    if not selected or len(selected) > 100:
        raise EvaluationError("Seleccione entre uno y 100 casos antes de procesar.")
    history = catalog.parent / ".evaluation-exposure"
    history.mkdir(exist_ok=True)
    lock = history / "lock"
    lock.mkdir()  # Exclusive across freezes using this catalog directory.
    try:
        exposed = set()
        for event in history.glob("*.json"):
            exposed.update(read_json(event)["sources"])
        if fresh and any(
            c["provenance"] != "new" or c["source_sha256"] in exposed or c["cohort"] == "known"
            for c in selected
        ):
            raise EvaluationError("La muestra nueva contiene procedencia pendiente o ya utilizada.")
        plan = {
            "version": VERSION,
            "id": str(uuid4()),
            "created_at": utc_now(),
            "catalog_sha256": file_hash(catalog),
            "question": raw["question"],
            "acceptance": raw["acceptance"],
            "seed": seed,
            "fresh_declared": fresh,
            "fresh_scope": "declaration_and_this_local_history_only",
            "repetitions": repetitions,
            "cases": selected,
        }
        destination.mkdir(parents=True, exist_ok=False)
        write_json(
            history / f"{plan['id']}.json",
            {"plan_id": plan["id"], "sources": sorted({c["source_sha256"] for c in selected})},
        )
        write_json(destination / "plan.json", plan)
        (destination / "plan.sha256").write_text(digest(plan), encoding="ascii")
        return plan
    finally:
        lock.rmdir()


def load_plan(root: Path) -> dict[str, Any]:
    plan = read_json(root / "plan.json")
    if not isinstance(plan, dict) or digest(plan) != (root / "plan.sha256").read_text(
        encoding="ascii"
    ):
        raise EvaluationError("El plan congelado fue modificado.")
    return plan


def runtime_identity() -> dict[str, Any]:
    files = [
        *sorted((ROOT / "src").rglob("*.py")),
        *sorted((ROOT / "scripts").glob("*.py")),
        ROOT / "pyproject.toml",
        ROOT / "requirements.lock",
        ROOT / "requirements-windows-cpu.lock",
        ROOT / "scripts" / "evaluation_review.html",
    ]
    return {
        "code_sha256": digest([(p.relative_to(ROOT).as_posix(), file_hash(p)) for p in files]),
        "environment_sha256": digest(
            sorted((d.metadata["Name"], d.version) for d in importlib.metadata.distributions())
        ),
        "python": platform.python_version(),
        "system": platform.system(),
        "machine": platform.machine(),
    }


def _ai_settings(enabled: bool) -> tuple[Any, dict[str, Any]]:
    if not enabled:
        return None, {}
    from liblevo.component_catalog import PRODUCT_COMPONENT_CATALOG
    from liblevo.local_ai_policy import ComponentCapability, verify_component_manifest
    from liblevo.local_models import is_ollama_local_only_configured
    from liblevo.settings import AppSettings

    if not is_ollama_local_only_configured():
        raise EvaluationError("Ollama necesita su protección local antes de evaluar documentos.")
    manifest = PRODUCT_COMPONENT_CATALOG[ComponentCapability.TRANSLATION].manifest
    verification = verify_component_manifest(manifest)
    if not verification.valid:
        raise EvaluationError("El componente local de traducción no está preparado.")
    return AppSettings(
        translation_model=manifest.model_name, translation_context_window=manifest.context_window
    ), {
        "model_digest": manifest.ollama_digest,
        "ollama_version": verification.ollama_version,
        "context_window": manifest.context_window,
    }


@contextmanager
def memory_sample() -> Iterator[dict[str, Any]]:
    """Optional process-tree RSS; explicitly excludes separately running Ollama/GPU."""
    import psutil

    sample: dict[str, Any] = {
        "peak_process_tree_rss_bytes": None,
        "memory_scope": "runner_and_children_excludes_ollama_and_gpu",
    }
    stop = threading.Event()

    def observe() -> None:
        while not stop.is_set():
            try:
                process = psutil.Process()
                amount = sum(
                    p.memory_info().rss for p in [process, *process.children(recursive=True)]
                )
                sample["peak_process_tree_rss_bytes"] = max(
                    amount, sample["peak_process_tree_rss_bytes"] or 0
                )
            except (psutil.Error, OSError):
                pass
            stop.wait(0.1)

    worker = threading.Thread(target=observe, daemon=True)
    worker.start()
    try:
        yield sample
    finally:
        stop.set()
        worker.join()


def _inventory(root: Path) -> dict[str, str]:
    files = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
            raise EvaluationError("La evidencia no admite enlaces fuera de su carpeta.")
        if path.is_file():
            files[path.relative_to(root).as_posix()] = file_hash(path)
    return files


def _process(case: dict[str, Any], output: Path, checkpoints: Path) -> Any:
    from liblevo.improvement_contracts import ImprovementMode
    from liblevo.pdf_conversion import PdfPageRange
    from liblevo.pipeline.contracts import ProcessRequest
    from liblevo.processing import process_document
    from liblevo.workflow import OutputFormat

    options = case["options"]
    settings, _ = _ai_settings(bool(options["translate_to"]))
    request = ProcessRequest(
        source_path=Path(case["source"]),
        convert_to_markdown=True,
        output_directory=output,
        output_format=OutputFormat(options["output_format"]),
        improvement_mode=ImprovementMode.TRANSLATE if options["translate_to"] else None,
        target_language=options["translate_to"],
        force_pdf_ocr=options["force_ocr"],
        include_images=options["include_images"],
        pdf_page_range=PdfPageRange(*options["pages"]) if options["pages"] else None,
    )
    return process_document(
        request,
        settings=settings,
        epub_checkpoint_root=checkpoints / "epub",
        work_checkpoint_root=checkpoints / "work",
    )


def run(root: Path, arm: str) -> None:
    if arm not in {"a", "b"}:
        raise EvaluationError("Seleccione una versión a o b.")
    root = root.resolve(strict=True)
    plan = load_plan(root)
    run_root = root / arm
    run_root.mkdir(exist_ok=False)
    identity = runtime_identity()
    uses_ai = any(c["options"]["translate_to"] for c in plan["cases"])
    try:
        _, models = _ai_settings(uses_ai)
    except EvaluationError:
        models = {"unavailable": True}
    write_evidence(
        run_root / "identity.json",
        {
            **identity,
            "models": models,
            "plan_sha256": digest(plan),
            "started_at": utc_now(),
            "cache": "isolated_per_case_and_repetition",
            "order": [c["id"] for c in plan["cases"]],
        },
    )
    for case in plan["cases"]:
        for repetition in range(plan["repetitions"]):
            unit = run_root / f"{case['id']}-{repetition}"
            unit.mkdir()
            output = unit / "output"
            output.mkdir()
            record: dict[str, Any] = {
                "case_id": case["id"],
                "repetition": repetition,
                "status": "failed",
                "source_unchanged": False,
                "integrity_verified": False,
                "requires_review": True,
                "reference_status": case["reference"]["status"],
                "literal_observations": [],
                "artifact": None,
            }
            started = time.monotonic()
            with memory_sample() as memory:
                try:
                    if case["options"]["translate_to"] and models.get("unavailable"):
                        raise EvaluationError("El componente de traducción no está preparado.")
                    if file_hash(Path(case["source"])) != case["source_sha256"]:
                        raise EvaluationError("Un original cambió desde la congelación.")
                    result = _process(case, output, unit / "checkpoints")
                    artifact = result.final_path.resolve(strict=True)
                    if not artifact.is_relative_to(output.resolve()):
                        raise EvaluationError("El resultado está fuera del espacio de evaluación.")
                    record["artifact"] = artifact.relative_to(unit).as_posix()
                    record["status"] = "completed"
                    record["integrity_verified"] = bool(
                        result.final_integrity_report and result.final_integrity_report.verified
                    )
                    report = result.translation_quality_for_review
                    record["requires_review"] = bool(
                        result.review_required
                        or result.preserved_translation_chunks
                        or (report and report.total_issues)
                        or result.problematic_pdf_pages
                    )
                    if (
                        artifact.suffix == ".md"
                        and case["reference"]["contains"]
                        and artifact.stat().st_size <= 16 * 1024 * 1024
                    ):
                        text = artifact.read_text(encoding="utf-8")
                        record["literal_observations"] = [
                            expected in text for expected in case["reference"]["contains"]
                        ]
                except Exception:
                    record["failure_code"] = "processing_failed"  # Never persist exception text.
                finally:
                    try:
                        record["source_unchanged"] = (
                            file_hash(Path(case["source"])) == case["source_sha256"]
                        )
                    except OSError:
                        record["source_unchanged"] = False
                    record["elapsed_seconds"] = round(time.monotonic() - started, 4)
                    record["files"] = _inventory(output)
                    record.update(memory)
                    write_evidence(unit / "result.json", record)
    try:
        _, final_models = _ai_settings(uses_ai)
    except EvaluationError:
        final_models = {"unavailable": True}
    unchanged = runtime_identity() == identity and final_models == models
    write_evidence(
        run_root / "finished.json", {"identity_unchanged": unchanged, "finished_at": utc_now()}
    )


def records(root: Path, plan: dict[str, Any], arm: str) -> list[dict[str, Any]]:
    result = []
    identity_path = root / arm / "identity.json"
    identity = read_evidence(identity_path) if evidence_complete(identity_path) else None
    if identity is not None and identity["plan_sha256"] != digest(plan):
        raise EvaluationError("Las ejecuciones no pertenecen al mismo plan.")
    for case in plan["cases"]:
        for repetition in range(plan["repetitions"]):
            unit = root / arm / f"{case['id']}-{repetition}"
            path = unit / "result.json"
            if identity is None or not evidence_complete(path):
                result.append(
                    {"case_id": case["id"], "repetition": repetition, "status": "incomplete"}
                )
                continue
            item = read_evidence(path)
            if item["case_id"] != case["id"] or item["repetition"] != repetition:
                raise EvaluationError("Un resultado no corresponde a su unidad.")
            if _inventory(unit / "output") != item["files"]:
                raise EvaluationError("Un resultado materializado fue modificado.")
            result.append(item)
    return result


def prepare_review(root: Path) -> Path:
    from scripts.evaluation_preview import document_preview, original_preview

    plan = load_plan(root)
    sides = {arm: records(root, plan, arm) for arm in ("a", "b")}
    review_id = str(uuid4())
    target = root / "reviews" / review_id
    view = target / "view"
    view.mkdir(parents=True)
    cards, mapping = [], {}
    for index, case in enumerate(plan["cases"]):
        source = Path(case["source"])
        if file_hash(source) != case["source_sha256"]:
            raise EvaluationError("El original cambió antes de la revisión.")
        original_name = f"original-{index}{source.suffix.lower()}"
        shutil.copyfile(source, view / original_name)
        source_html, source_full = original_preview(source, case["options"]["pages"], view, index)
        for repetition in range(plan["repetitions"]):
            arms = ["a", "b"]
            if secrets.randbelow(2):
                arms.reverse()
            panels = []
            for arm in arms:
                record = sides[arm][index * plan["repetitions"] + repetition]
                panel_id = str(uuid4())
                mapping[panel_id] = {
                    "arm": arm,
                    "case_id": case["id"],
                    "repetition": repetition,
                    "record_sha256": digest(record),
                }
                panel = {
                    "id": panel_id,
                    "html": "<p>No hay resultado completo.</p>",
                    "file": None,
                    "needs_full": True,
                    "completed": record["status"] == "completed",
                }
                if record.get("artifact"):
                    unit = root / arm / f"{case['id']}-{repetition}"
                    shutil.copytree(unit / "output", view / panel_id)
                    artifact = Path(record["artifact"]).relative_to("output")
                    panel["file"] = f"{panel_id}/{artifact.as_posix()}"
                    panel["html"], panel["needs_full"] = document_preview(
                        view / panel_id / artifact
                    )
                panels.append(panel)
            for panel in panels:
                panel["ids"] = [panel["id"]]
            left, right = (sides[arm][index * plan["repetitions"] + repetition] for arm in arms)
            if (
                all(panel["completed"] for panel in panels)
                and left["artifact"] == right["artifact"]
                and left["files"] == right["files"]
            ):
                panels[0]["ids"].append(panels[1]["id"])
                panels = panels[:1]
            for panel in panels:
                for panel_id in panel["ids"]:
                    mapping[panel_id]["review_group"] = panel["id"]
            cards.append(
                {
                    "case": case["id"],
                    "repetition": repetition,
                    "pages": case["options"]["pages"],
                    "output_format": case["options"]["output_format"],
                    "translate_to": case["options"]["translate_to"],
                    "source": original_name,
                    "source_html": source_html,
                    "source_needs_full": source_full,
                    "panels": panels,
                }
            )
    payload = {
        "review_id": review_id,
        "plan_sha256": digest(plan),
        "practice": all(case["provenance"] == "synthetic" for case in plan["cases"]),
        "cards": cards,
    }
    payload_hash = digest(payload)
    write_evidence(target / "mapping.json", {"payload_sha256": payload_hash, "panels": mapping})
    payload["payload_sha256"] = payload_hash
    encoded = json.dumps(payload, ensure_ascii=True).replace("<", "\\u003c").replace("&", "\\u0026")
    template = (ROOT / "scripts" / "evaluation_review.html").read_text(encoding="utf-8")
    (view / "index.html").write_text(template.replace("__PAYLOAD__", encoded), encoding="utf-8")
    write_evidence(target / "view-hashes.json", _inventory(view))
    return view / "index.html"


def import_review(root: Path, answers: Path) -> Path:
    plan = load_plan(root)
    raw = read_json(answers)
    review_id = _uuid(raw.get("review_id"))
    target = root / "reviews" / review_id
    mapping = read_evidence(target / "mapping.json")
    if (
        raw.get("plan_sha256") != digest(plan)
        or raw.get("payload_sha256") != mapping["payload_sha256"]
    ):
        raise EvaluationError("La revisión corresponde a otro paquete.")
    if _inventory(target / "view") != read_evidence(target / "view-hashes.json"):
        raise EvaluationError("El paquete presentado a revisión fue modificado.")
    verdicts = raw.get("verdicts")
    if not isinstance(verdicts, list) or len(verdicts) != len(mapping["panels"]):
        raise EvaluationError("Revise todos los paneles o márquelos como no evaluables.")
    ids = [v.get("panel_id") for v in verdicts]
    if len(set(ids)) != len(ids) or set(ids) != set(mapping["panels"]):
        raise EvaluationError("Hay paneles omitidos, duplicados o ajenos.")
    by_arm = {arm: records(root, plan, arm) for arm in ("a", "b")}
    accepted = []
    for verdict in verdicts:
        if verdict.get("label") not in LABELS or (
            verdict.get("label") != "not_evaluable"
            and (
                verdict.get("source_inspected") is not True
                or verdict.get("full_result_inspected") is not True
            )
        ):
            raise EvaluationError("Cada juicio necesita una etiqueta y el original revisado.")
        seconds = verdict.get("review_seconds")
        comment = verdict.get("comment", "")
        if not isinstance(comment, str) or len(comment) > 2000 or "\0" in comment:
            raise EvaluationError("Comentario no válido: máximo 2000 caracteres.")
        if seconds is not None and (
            type(seconds) not in {int, float}
            or not math.isfinite(seconds)
            or not 0 <= seconds <= 86400
        ):
            raise EvaluationError("Tiempo de revisión no válido.")
        reference = mapping["panels"][verdict["panel_id"]]
        record = next(
            r
            for r in by_arm[reference["arm"]]
            if r["case_id"] == reference["case_id"] and r["repetition"] == reference["repetition"]
        )
        if digest(record) != reference["record_sha256"]:
            raise EvaluationError("El resultado cambió después de preparar la revisión.")
        if record["status"] != "completed" and verdict["label"] != "not_evaluable":
            raise EvaluationError("Un resultado incompleto no admite aprobación de contenido.")
        accepted.append(
            {**reference, "label": verdict["label"], "review_seconds": seconds, "comment": comment}
        )
    event_id = str(uuid4())
    events = root / "judgments"
    events.mkdir(exist_ok=True)
    destination = events / f"{event_id}.json"
    write_evidence(
        destination,
        {
            "id": event_id,
            "review_id": review_id,
            "created_at": utc_now(),
            "answers_sha256": file_hash(answers),
            "verdicts": accepted,
        },
    )
    return destination


def report(root: Path, *, judgment: Path | None = None) -> dict[str, Any]:
    plan = load_plan(root)
    evaluated = {arm: records(root, plan, arm) for arm in ("a", "b")}
    verdicts = []
    if judgment is not None:
        # Only an explicit accepted event: a later review never silently replaces an earlier one.
        event = read_evidence(judgment)
        if judgment.resolve() != (root / "judgments" / f"{_uuid(event['id'])}.json").resolve():
            raise EvaluationError("Seleccione un evento de revisión importado en este plan.")
        verdicts = event["verdicts"]
        for v in verdicts:
            current = next(
                r
                for r in evaluated[v["arm"]]
                if r["case_id"] == v["case_id"] and r["repetition"] == v["repetition"]
            )
            if digest(current) != v["record_sha256"]:
                raise EvaluationError("La revisión ya no coincide con los resultados.")
    summary: dict[str, Any] = {
        "version": VERSION,
        "plan_id": plan["id"],
        "plan_sha256": digest(plan),
        "decision": "human_decision_required",
        "semantic_autoapproval": False,
        "human_review_units": len(
            {v.get("review_group", (v["arm"], v["case_id"], v["repetition"])) for v in verdicts}
        ),
        "fresh_declared": plan["fresh_declared"],
        "fresh_scope": plan["fresh_scope"],
        "cases": len(plan["cases"]),
        "source_documents": len({c["source_sha256"] for c in plan["cases"]}),
        "repetitions": plan["repetitions"],
        "arms": {},
        "limits": [
            "No population accuracy estimate",
            "No installer or UI guarantee from this runner",
            "Memory excludes separately running Ollama and GPU",
            "Human labels are attestations",
        ],
    }
    cases = {c["id"]: c for c in plan["cases"]}
    for arm, results in evaluated.items():
        finished = root / arm / "finished.json"
        stable = (
            evidence_complete(finished)
            and read_evidence(finished).get("identity_unchanged") is True
        )
        groups = {}
        for cohort in sorted(COHORTS):
            units = [r for r in results if cases[r["case_id"]]["cohort"] == cohort]
            labels = [
                v for v in verdicts if v["arm"] == arm and cases[v["case_id"]]["cohort"] == cohort
            ]
            elapsed = [r["elapsed_seconds"] for r in units if "elapsed_seconds" in r]
            groups[cohort] = {
                "requested": len(units),
                "statuses": dict(Counter(r["status"] for r in units)),
                "integrity_verified": sum(r.get("integrity_verified", False) for r in units),
                "source_changed": sum(r.get("source_unchanged") is False for r in units),
                "requires_review": sum(r.get("requires_review", True) for r in units),
                "reference_statuses": dict(
                    Counter(cases[r["case_id"]]["reference"]["status"] for r in units)
                ),
                "literal_mismatches": sum(
                    r.get("literal_observations", []).count(False) for r in units
                ),
                "literal_checks_executed": sum(
                    len(r.get("literal_observations", [])) for r in units
                ),
                "literal_checks_requested": sum(
                    len(cases[r["case_id"]]["reference"]["contains"]) for r in units
                ),
                "human_labels": dict(Counter(v["label"] for v in labels)),
                "pending_human": len(units) - len(labels),
                "review_seconds": (
                    sum(v["review_seconds"] for v in labels)
                    if labels and all(v["review_seconds"] is not None for v in labels)
                    else None
                ),
                "elapsed_median_seconds": median(elapsed) if elapsed else None,
                "elapsed_max_seconds": max(elapsed) if elapsed else None,
                "peak_process_tree_rss_bytes": max(
                    (r.get("peak_process_tree_rss_bytes") or 0 for r in units), default=0
                )
                or None,
            }
        identity_path = root / arm / "identity.json"
        summary["arms"][arm] = {
            "identity_stable": stable,
            "groups": groups,
            "identity": read_evidence(identity_path) if evidence_complete(identity_path) else None,
        }
        if not stable or any(
            r["status"] != "completed"
            or not r.get("source_unchanged")
            or not r.get("integrity_verified")
            for r in results
        ):
            summary["decision"] = "technical_failure_or_incomplete"
    if (not verdicts or any(v["label"] == "not_evaluable" for v in verdicts)) and summary[
        "decision"
    ] == "human_decision_required":
        summary["decision"] = "insufficient_evidence"
    return summary


def write_report(path: Path, summary: dict[str, Any]) -> None:
    readable = path.with_suffix(".html")
    if path.exists() or readable.exists() or path.suffix != ".json":
        raise EvaluationError("El informe necesita un nombre .json nuevo.")
    write_json(path, summary)
    states = {
        "human_decision_required": "Pendiente de decisión humana",
        "technical_failure_or_incomplete": "Hay fallos técnicos o ejecuciones incompletas",
        "insufficient_evidence": "Evidencia insuficiente",
    }
    rows = []
    for arm, value in summary["arms"].items():
        for group, values in value["groups"].items():
            if not values["requested"]:
                continue
            labels = values["human_labels"]
            rows.append(
                "<tr>"
                + "".join(
                    f"<td>{html.escape(str(v)) if v is not None else 'Sin medir'}</td>"
                    for v in (
                        arm.upper(),
                        {
                            "known": "Conocidos",
                            "risk": "Riesgo",
                            "representative": "Representativa",
                        }[group],
                        values["requested"],
                        values["statuses"].get("completed", 0),
                        labels.get("major", 0),
                        labels.get("minor", 0),
                        values["pending_human"] + labels.get("not_evaluable", 0),
                        values["elapsed_median_seconds"],
                        values["review_seconds"],
                    )
                )
                + "</tr>"
            )
    document = (
        '<!doctype html><html lang="es"><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width,initial-scale=1">'
        "<title>Liblevo · Informe de evaluación</title><style>"
        "body{font:16px/1.5 system-ui;max-width:1100px;margin:30px auto;padding:16px}"
        "table{border-collapse:collapse}td,th{padding:10px;border:1px solid #bac7ce}"
        ".table{overflow:auto}</style><h1>Evaluación comparativa</h1>"
        f"<p><strong>{states[summary['decision']]}</strong></p>"
        f"<p>{summary['cases']} casos de {summary['source_documents']} originales; "
        f"{summary['repetitions']} repetición/es. No estima precisión general.</p>"
        '<div class="table"><table><tr><th>Versión</th><th>Grupo</th><th>Solicitados</th>'
        "<th>Completados</th><th>Errores graves</th><th>Menores</th><th>Sin valorar</th>"
        "<th>Tiempo mediano (s)</th><th>Revisión humana (s)</th></tr>"
        + "".join(rows)
        + "</table></div><p>Cero errores con casos sin valorar no "
        "demuestra calidad. Los contratos del procesador no sustituyen "
        "la revisión del original.</p>"
        "<p>Los datos completos, identidades y límites de memoria están en el JSON compañero. "
        "Este informe no aprueba documentos ni publica una versión de la aplicación.</p></html>"
    )
    with readable.open("x", encoding="utf-8") as stream:
        stream.write(document)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Evaluación local; ningún resultado se autoaprueba."
    )
    commands = parser.add_subparsers(dest="command", required=True)
    init = commands.add_parser("init")
    init.add_argument("directory", type=Path)
    frozen = commands.add_parser("freeze")
    frozen.add_argument("catalog", type=Path)
    frozen.add_argument("destination", type=Path)
    for group in ("representative", "risk", "known"):
        frozen.add_argument(f"--{group}", type=int, default=0)
    frozen.add_argument("--seed", type=int, required=True)
    frozen.add_argument("--fresh", action="store_true")
    frozen.add_argument("--repetitions", type=int, default=1)
    execution = commands.add_parser("run")
    execution.add_argument("plan", type=Path)
    execution.add_argument("--arm", choices=("a", "b"), required=True)
    review = commands.add_parser("review")
    review.add_argument("plan", type=Path)
    accept = commands.add_parser("import-review")
    accept.add_argument("plan", type=Path)
    accept.add_argument("answers", type=Path)
    output = commands.add_parser("report")
    output.add_argument("plan", type=Path)
    output.add_argument("destination", type=Path)
    output.add_argument("--judgment", type=Path)
    args = parser.parse_args()
    logging.disable(logging.CRITICAL)
    try:
        if args.command == "init":
            args.directory.mkdir(parents=True, exist_ok=False)
            write_json(
                args.directory / "catalog.json",
                {"version": VERSION, "question": "", "acceptance": "", "cases": []},
            )
        elif args.command == "freeze":
            freeze(
                args.catalog,
                args.destination,
                representative=args.representative,
                risk=args.risk,
                known=args.known,
                seed=args.seed,
                fresh=args.fresh,
                repetitions=args.repetitions,
            )
        elif args.command == "run":
            run(args.plan, args.arm)
        elif args.command == "review":
            path = prepare_review(args.plan)
            print(f"Revisión privada preparada. Identificador: {path.parents[1].name}")
        elif args.command == "import-review":
            path = import_review(args.plan, args.answers)
            print(f"Decisión humana registrada. Identificador: {path.stem}")
        else:
            write_report(args.destination, report(args.plan, judgment=args.judgment))
        print("Operación completada. No implica aprobación semántica ni publicación.")
        return 0
    except EvaluationError as exc:
        print(f"Evaluación no completada: {exc}")
        return 1
    except (Exception, KeyboardInterrupt):
        print("Evaluación no completada. Revise entradas, identidades y permisos.")
        return 1


if __name__ == "__main__":
    sys.path.insert(0, str(ROOT))
    sys.exit(main())
