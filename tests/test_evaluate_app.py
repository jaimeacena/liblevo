from __future__ import annotations

import json
from pathlib import Path
from types import SimpleNamespace
from uuid import uuid4

import pytest
from scripts import evaluate_app as evaluation


def catalog(tmp_path: Path, *, provenance="synthetic", cohort="known", content=None):
    source = tmp_path / "private-original.md"
    source.write_text(
        content or "# Informe\n\nLa cantidad es 25.\n\nTexto completo.\n", encoding="utf-8"
    )
    case = {
        "id": str(uuid4()),
        "source": source.name,
        "source_sha256": evaluation.file_hash(source),
        "cohort": cohort,
        "provenance": provenance,
        "options": {"include_images": False},
        "reference": {
            "status": "unverified",
            "contains": ["La cantidad es 25.", "Texto completo."],
        },
    }
    path = tmp_path / "catalog.json"
    evaluation.write_json(
        path,
        {
            "version": 1,
            "question": "¿Conserva el contenido?",
            "acceptance": "Sin pérdidas nuevas.",
            "cases": [case],
        },
    )
    return path, case


def frozen(tmp_path, **kwargs):
    path, case = catalog(tmp_path, **kwargs)
    root = tmp_path / "comparison"
    evaluation.freeze(
        path,
        root,
        representative=int(case["cohort"] == "representative"),
        risk=0,
        known=int(case["cohort"] == "known"),
        seed=7,
        fresh=False,
        repetitions=1,
    )
    return root, case


@pytest.fixture
def fast_runtime(monkeypatch):
    monkeypatch.setattr(evaluation, "runtime_identity", lambda: {"code_sha256": "a" * 64})

    def process(case, output, checkpoints):
        artifact = output / "result.md"
        artifact.write_text(Path(case["source"]).read_text(encoding="utf-8"), encoding="utf-8")
        return SimpleNamespace(
            final_path=artifact,
            final_integrity_report=SimpleNamespace(verified=True),
            translation_quality_for_review=None,
            review_required=False,
            preserved_translation_chunks=(),
            problematic_pdf_pages=(),
        )

    monkeypatch.setattr(evaluation, "_process", process)


def answers(root, *, label="no_error_observed"):
    page = evaluation.prepare_review(root)
    target = page.parents[1]
    mapping = evaluation.read_evidence(target / "mapping.json")
    payload = {
        "review_id": target.name,
        "plan_sha256": evaluation.digest(evaluation.load_plan(root)),
        "payload_sha256": mapping["payload_sha256"],
        "verdicts": [
            {
                "panel_id": key,
                "label": label,
                "source_inspected": True,
                "full_result_inspected": True,
                "review_seconds": None,
            }
            for key in mapping["panels"]
        ],
    }
    path = root / f"answers-{uuid4()}.json"
    evaluation.write_json(path, payload)
    return path, payload, page


def test_empty_catalog_never_adopts_existing_corpus(tmp_path):
    path = tmp_path / "catalog.json"
    evaluation.write_json(path, {"version": 1, "cases": []})
    with pytest.raises(evaluation.EvaluationError, match="uno y 500"):
        evaluation.load_catalog(path)


def test_ai_evaluation_requires_local_privacy_before_model_metadata(monkeypatch):
    import parsezen.local_ai_policy as policy
    import parsezen.local_models as models

    monkeypatch.setattr(models, "is_ollama_local_only_configured", lambda: False)

    def forbidden(_manifest):
        pytest.fail("No metadata request before local privacy is configured")

    monkeypatch.setattr(policy, "verify_component_manifest", forbidden)
    with pytest.raises(evaluation.EvaluationError, match="protección local"):
        evaluation._ai_settings(True)


def test_ai_evaluation_uses_only_the_verified_fixed_component(monkeypatch):
    import parsezen.local_ai_policy as policy
    import parsezen.local_models as models
    from parsezen.component_catalog import PRODUCT_COMPONENT_CATALOG

    monkeypatch.setattr(models, "is_ollama_local_only_configured", lambda: True)
    inspected = []

    def verified(manifest):
        inspected.append(manifest)
        return SimpleNamespace(valid=True, ollama_version="0.17.4")

    monkeypatch.setattr(policy, "verify_component_manifest", verified)
    settings, identity = evaluation._ai_settings(True)
    expected = PRODUCT_COMPONENT_CATALOG[policy.ComponentCapability.TRANSLATION].manifest
    assert inspected == [expected]
    assert settings.translation_model == expected.model_name
    assert identity["model_digest"] == expected.ollama_digest


@pytest.mark.parametrize("provenance", ["unverified", "known", "synthetic"])
def test_fresh_sample_rejects_unverified_or_used_provenance(tmp_path, provenance):
    path, _ = catalog(tmp_path, provenance=provenance, cohort="representative")
    with pytest.raises(evaluation.EvaluationError, match="pendiente o ya utilizada"):
        evaluation.freeze(
            path,
            tmp_path / "plan",
            representative=1,
            risk=0,
            known=0,
            seed=1,
            fresh=True,
            repetitions=1,
        )
    assert not (tmp_path / "plan").exists()


def test_new_case_id_cannot_reuse_same_original_as_fresh(tmp_path):
    path, _ = catalog(tmp_path, provenance="new", cohort="representative")
    evaluation.freeze(
        path,
        tmp_path / "first",
        representative=1,
        risk=0,
        known=0,
        seed=1,
        fresh=True,
        repetitions=1,
    )
    raw = evaluation.read_json(path)
    raw["cases"][0]["id"] = str(uuid4())
    path.write_text(json.dumps(raw), encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError, match="pendiente o ya utilizada"):
        evaluation.freeze(
            path,
            tmp_path / "second",
            representative=1,
            risk=0,
            known=0,
            seed=1,
            fresh=True,
            repetitions=1,
        )


def test_expected_reference_needs_original_and_expectation_attestation(tmp_path):
    path, _ = catalog(tmp_path)
    raw = evaluation.read_json(path)
    raw["cases"][0]["reference"]["status"] = "human_verified"
    path.write_text(json.dumps(raw), encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError, match="no acredita"):
        evaluation.load_catalog(path)


def test_source_and_plan_modification_are_detected_before_execution(tmp_path):
    path, case = catalog(tmp_path)
    Path(tmp_path / case["source"]).write_text("Changed", encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError, match="original"):
        evaluation.load_catalog(path)


def test_frozen_plan_and_duplicate_ids_are_rejected(tmp_path):
    root, _ = frozen(tmp_path)
    plan = evaluation.read_json(root / "plan.json")
    plan["repetitions"] = 2
    (root / "plan.json").write_text(json.dumps(plan), encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError, match="modificado"):
        evaluation.load_plan(root)
    raw = evaluation.read_json(tmp_path / "catalog.json")
    raw["cases"].append(raw["cases"][0])
    (tmp_path / "catalog.json").write_text(json.dumps(raw), encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError, match="duplicados"):
        evaluation.load_catalog(tmp_path / "catalog.json")


def test_identical_outputs_and_clean_guards_never_autoapprove(tmp_path, fast_runtime):
    root, _ = frozen(tmp_path)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    summary = evaluation.report(root)
    assert summary["decision"] == "insufficient_evidence"
    assert summary["semantic_autoapproval"] is False
    group = summary["arms"]["a"]["groups"]["known"]
    assert group["literal_mismatches"] == 0
    assert group["reference_statuses"] == {"unverified": 1}
    assert group["pending_human"] == 1
    with pytest.raises(FileExistsError):
        evaluation.run(root, "a")


def test_failed_and_never_started_attempts_remain_in_denominator(
    tmp_path, fast_runtime, monkeypatch
):
    root, _ = frozen(tmp_path)

    def fail(*_args):
        raise RuntimeError("DO NOT EXPORT private-document-text C:/private/path")

    monkeypatch.setattr(evaluation, "_process", fail)
    evaluation.run(root, "a")
    summary = evaluation.report(root)
    assert summary["decision"] == "technical_failure_or_incomplete"
    assert summary["arms"]["a"]["groups"]["known"]["statuses"] == {"failed": 1}
    assert summary["arms"]["b"]["groups"]["known"]["statuses"] == {"incomplete": 1}
    serialized = json.dumps(summary)
    assert "private-document" not in serialized
    assert str(tmp_path) not in serialized


def test_changed_artifact_and_forged_result_are_rejected(tmp_path, fast_runtime):
    root, case = frozen(tmp_path)
    evaluation.run(root, "a")
    artifact = root / "a" / f"{case['id']}-0" / "output" / "result.md"
    artifact.write_text("different", encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError, match="materializado"):
        evaluation.report(root)
    result = artifact.parents[1] / "result.json"
    result.write_text("{}", encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError, match="evidencia"):
        evaluation.report(root)


def test_interruption_between_record_and_receipt_counts_as_incomplete(tmp_path, fast_runtime):
    root, case = frozen(tmp_path)
    (root / "a").mkdir()
    evaluation.write_evidence(
        root / "a" / "identity.json",
        {
            "plan_sha256": evaluation.digest(evaluation.load_plan(root)),
        },
    )
    unit = root / "a" / f"{case['id']}-0"
    unit.mkdir()
    # A partial write is not a completed attempt even if it resembles one.
    evaluation.write_json(unit / "result.json", {"status": "completed"})
    summary = evaluation.report(root)
    assert summary["arms"]["a"]["groups"]["known"]["statuses"] == {"incomplete": 1}
    assert summary["decision"] == "technical_failure_or_incomplete"


def test_review_is_blinded_escaped_bound_and_never_prepicks_approval(tmp_path, fast_runtime):
    root, _ = frozen(tmp_path, content="</script><script>alert('private')</script>")
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    path, payload, page = answers(root)
    document = page.read_text(encoding="utf-8")
    assert "</script><script>alert" not in document
    assert '"arm":' not in document
    assert "Necesito ayuda" in document
    assert "connect-src 'none'" in document
    accepted = evaluation.import_review(root, path)
    summary = evaluation.report(root, judgment=accepted)
    assert summary["decision"] == "human_decision_required"
    assert summary["arms"]["a"]["groups"]["known"]["human_labels"] == {"no_error_observed": 1}
    assert summary["arms"]["a"]["groups"]["known"]["review_seconds"] is None
    assert "private" not in json.dumps(summary)
    # Later corrections create a distinct event; the old decision remains unchanged.
    payload["verdicts"][0]["label"] = "major"
    path.write_text(json.dumps(payload), encoding="utf-8")
    corrected = evaluation.import_review(root, path)
    assert corrected != accepted
    assert evaluation.read_evidence(accepted)["verdicts"][0]["label"] == "no_error_observed"


@pytest.mark.parametrize("provenance", ["synthetic", "unverified"])
def test_guided_review_distinguishes_practice_and_describes_requested_task(
    tmp_path, fast_runtime, provenance
):
    root, _ = frozen(tmp_path, provenance=provenance)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    page = evaluation.prepare_review(root)
    document = page.read_text(encoding="utf-8")
    embedded = json.loads(
        document.split('<script id="data" type="application/json">', 1)[1].split("</script>", 1)[0]
    )
    assert embedded["practice"] is (provenance == "synthetic")
    assert embedded["cards"][0]["output_format"] == "markdown"
    assert embedded["cards"][0]["translate_to"] is None
    assert "arm" not in embedded["cards"][0]["panels"][0]


def test_identical_outputs_share_one_review_and_comments_stay_private(tmp_path, fast_runtime):
    root, _ = frozen(tmp_path)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    path, payload, page = answers(root)
    document = page.read_text(encoding="utf-8")
    embedded = json.loads(
        document.split('<script id="data" type="application/json">', 1)[1].split("</script>", 1)[0]
    )
    assert len(embedded["cards"][0]["panels"]) == 1
    assert set(embedded["cards"][0]["panels"][0]["ids"]) == {
        v["panel_id"] for v in payload["verdicts"]
    }
    for verdict in payload["verdicts"]:
        verdict["comment"] = "Comentario privado: falta una fila."
    path.write_text(json.dumps(payload), encoding="utf-8")
    event = evaluation.import_review(root, path)
    assert all(
        v["comment"] == "Comentario privado: falta una fila."
        for v in evaluation.read_evidence(event)["verdicts"]
    )
    summary = evaluation.report(root, judgment=event)
    assert summary["human_review_units"] == 1
    assert "Comentario privado" not in json.dumps(summary)


def test_direct_cli_can_prepare_review_from_another_directory(tmp_path, fast_runtime):
    import subprocess
    import sys

    root, _ = frozen(tmp_path)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    result = subprocess.run(
        [sys.executable, str(evaluation.ROOT / "scripts/evaluate_app.py"), "review", str(root)],
        cwd=tmp_path,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 0


def test_same_text_with_different_resources_is_not_grouped(tmp_path, fast_runtime, monkeypatch):
    root, _ = frozen(tmp_path)
    process = evaluation._process

    def differing_resource(case, output, checkpoints):
        result = process(case, output, checkpoints)
        (output / "resource.txt").write_text("a" if "a" in output.parts else "b", encoding="utf-8")
        return result

    monkeypatch.setattr(evaluation, "_process", differing_resource)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    page = evaluation.prepare_review(root)
    document = page.read_text(encoding="utf-8")
    embedded = json.loads(
        document.split('<script id="data" type="application/json">', 1)[1].split("</script>", 1)[0]
    )
    assert len(embedded["cards"][0]["panels"]) == 2


@pytest.mark.parametrize("comment", [None, 42, "x" * 2001, "bad\0comment"])
def test_invalid_comments_are_not_imported(tmp_path, fast_runtime, comment):
    root, _ = frozen(tmp_path)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    path, payload, _ = answers(root)
    payload["verdicts"][0]["comment"] = comment
    path.write_text(json.dumps(payload), encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError, match="Comentario"):
        evaluation.import_review(root, path)


@pytest.mark.parametrize("mutation", ["duplicate", "missing", "uninspected", "wrong_hash", "nan"])
def test_invalid_human_answer_cannot_enter_evidence(tmp_path, fast_runtime, mutation):
    root, _ = frozen(tmp_path)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    path, payload, _ = answers(root)
    if mutation == "duplicate":
        payload["verdicts"][1] = payload["verdicts"][0]
    elif mutation == "missing":
        payload["verdicts"].pop()
    elif mutation == "uninspected":
        payload["verdicts"][0]["full_result_inspected"] = False
    elif mutation == "wrong_hash":
        payload["payload_sha256"] = "0" * 64
    else:
        payload["verdicts"][0]["review_seconds"] = float("nan")
    path.write_text(json.dumps(payload), encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError):
        evaluation.import_review(root, path)
    assert not (root / "judgments").exists()


def test_uncertainty_is_explicit_and_never_forces_an_assessment(tmp_path, fast_runtime):
    root, _ = frozen(tmp_path)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    path, payload, _ = answers(root, label="not_evaluable")
    for verdict in payload["verdicts"]:
        verdict["source_inspected"] = False
        verdict["full_result_inspected"] = False
    path.write_text(json.dumps(payload), encoding="utf-8")
    accepted = evaluation.import_review(root, path)
    assert evaluation.report(root, judgment=accepted)["decision"] == "insufficient_evidence"


def test_modified_private_review_page_is_rejected(tmp_path, fast_runtime):
    root, _ = frozen(tmp_path)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    path, _, page = answers(root)
    page.write_text("forged", encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError, match="paquete"):
        evaluation.import_review(root, path)


@pytest.mark.parametrize("degraded_arm", ["a", "b"])
def test_literal_probe_detects_omission_but_does_not_certify_semantics(
    tmp_path, fast_runtime, monkeypatch, degraded_arm
):
    root, _ = frozen(tmp_path)
    process = evaluation._process

    def omit(case, output, checkpoints):
        result = process(case, output, checkpoints)
        if degraded_arm in output.parts:
            result.final_path.write_text("# Informe\n\nLa cantidad es 25.\n", encoding="utf-8")
        return result

    monkeypatch.setattr(evaluation, "_process", omit)
    for arm in ("a", "b"):
        evaluation.run(root, arm)
    summary = evaluation.report(root)
    for arm in ("a", "b"):
        assert summary["arms"][arm]["groups"]["known"]["literal_mismatches"] == int(
            arm == degraded_arm
        )
    assert summary["decision"] == "insufficient_evidence"


def test_real_processor_and_readable_report_on_new_synthetic_original(tmp_path, monkeypatch):
    root, _ = frozen(tmp_path)
    monkeypatch.chdir(tmp_path)
    for arm in ("a", "b"):
        evaluation.run(Path("comparison"), arm)
    summary = evaluation.report(root)
    for arm in ("a", "b"):
        assert summary["arms"][arm]["identity_stable"]
        assert summary["arms"][arm]["groups"]["known"]["statuses"] == {"completed": 1}
        assert summary["arms"][arm]["groups"]["known"]["integrity_verified"] == 1
    destination = tmp_path / "report.json"
    evaluation.write_report(destination, summary)
    assert "Evidencia insuficiente" in destination.with_suffix(".html").read_text(encoding="utf-8")
    with pytest.raises(evaluation.EvaluationError):
        evaluation.write_report(destination, summary)
