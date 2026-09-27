import json
import subprocess
from pathlib import Path

import pytest
from scripts import audit_dependencies
from scripts.audit_dependencies import (
    locked_versions,
    validate_audit_coverage,
    validate_lock_alignment,
)


def test_locked_versions_reads_compiled_requirements(tmp_path: Path) -> None:
    lock = tmp_path / "requirements.lock"
    lock.write_text(
        "Torch==2.13.0+cpu \\\n    --hash=sha256:abc\nhttpx==0.28.1\n",
        encoding="utf-8",
    )

    assert locked_versions(lock) == {"torch": "2.13.0+cpu", "httpx": "0.28.1"}


def test_variant_skip_requires_an_audited_aligned_canonical_package() -> None:
    primary = {"dependencies": [{"name": "torch", "skip_reason": "not on PyPI"}]}
    canonical = {"dependencies": [{"name": "torch", "version": "2.13.0", "vulns": []}]}

    validate_audit_coverage(
        primary,
        primary_versions={"torch": "2.13.0+cpu"},
        allowed_variant_skips=frozenset({"torch"}),
        canonical=canonical,
        canonical_versions={"torch": "2.13.0"},
    )


def test_unexpected_or_misaligned_skips_fail_closed() -> None:
    primary = {"dependencies": [{"name": "torch", "skip_reason": "not on PyPI"}]}

    with pytest.raises(ValueError, match="omisiones inesperadas"):
        validate_audit_coverage(primary, primary_versions={"torch": "2.13.0+cpu"})

    with pytest.raises(ValueError, match="no coincide"):
        validate_audit_coverage(
            primary,
            primary_versions={"torch": "2.13.0+cpu"},
            allowed_variant_skips=frozenset({"torch"}),
            canonical={"dependencies": [{"name": "torch", "version": "2.12.0", "vulns": []}]},
            canonical_versions={"torch": "2.12.0"},
        )


@pytest.mark.parametrize(
    "dependencies",
    [
        [],
        [{"name": "other", "version": "1.0", "vulns": []}],
        [{"name": "httpx", "version": "0.27.0", "vulns": []}],
        [{"name": "httpx", "version": "0.28.1"}],
        [None],
        [{"name": 42}],
        [{"name": "httpx", "skip_reason": True}],
        [{"name": "httpx", "version": "0.28.1", "vulns": []}] * 2,
    ],
)
def test_incomplete_or_invalid_inventory_is_rejected(dependencies: list[object]) -> None:
    with pytest.raises(ValueError):
        validate_audit_coverage(
            {"dependencies": dependencies}, primary_versions={"httpx": "0.28.1"}
        )


def test_complete_canonical_inventory_is_checked_even_without_primary_skips() -> None:
    with pytest.raises(ValueError, match="faltan: httpx"):
        validate_audit_coverage(
            {"dependencies": [{"name": "httpx", "version": "0.28.1", "vulns": []}]},
            primary_versions={"httpx": "0.28.1"},
            canonical={"dependencies": []},
            canonical_versions={"httpx": "0.28.1"},
        )


@pytest.mark.parametrize("content", ["# empty\n", "Foo-Bar==1\nfoo_bar==1\n"])
def test_empty_and_duplicate_locks_are_rejected(tmp_path: Path, content: str) -> None:
    lock = tmp_path / "requirements.lock"
    lock.write_text(content, encoding="utf-8")
    with pytest.raises(ValueError):
        locked_versions(lock)


def test_committed_cpu_lock_matches_canonical_versions() -> None:
    root = Path(__file__).parents[1]
    validate_lock_alignment(
        locked_versions(root / "requirements-windows-cpu.lock"),
        locked_versions(root / "requirements.lock"),
    )


@pytest.mark.parametrize("version", ["2.12.0+cpu", "2.13.0+cu128"])
def test_unapproved_torch_variant_fails(version: str) -> None:
    with pytest.raises(ValueError, match="no coincide"):
        validate_lock_alignment({"torch": version}, {"torch": "2.13.0"})


def test_failed_audit_reports_package_version_and_identifier_without_raw_details(
    monkeypatch, tmp_path: Path
) -> None:
    payload = {
        "dependencies": [
            {
                "name": "example-package",
                "version": "1.0",
                "vulns": [{"id": "CVE-2099-12345", "description": "private detail"}],
            }
        ]
    }
    monkeypatch.setattr(
        audit_dependencies.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(
            [], 1, stdout=json.dumps(payload), stderr="private server path"
        ),
    )
    with pytest.raises(RuntimeError, match="example-package 1.0: CVE-2099-12345") as failure:
        audit_dependencies.run_audit(tmp_path / "requirements.lock", ())
    assert "private" not in str(failure.value)


def test_canonical_report_requires_its_expected_inventory() -> None:
    with pytest.raises(ValueError, match="Falta"):
        validate_audit_coverage(
            {"dependencies": [{"name": "httpx", "version": "0.28.1", "vulns": []}]},
            primary_versions={"httpx": "0.28.1"},
            canonical={"dependencies": []},
        )


def test_canonical_audit_cannot_hide_an_unrelated_skip() -> None:
    with pytest.raises(ValueError, match="auditoría canónica: torchvision"):
        validate_audit_coverage(
            {"dependencies": [{"name": "torch", "skip_reason": "variant"}]},
            primary_versions={"torch": "2.13.0+cpu"},
            allowed_variant_skips=frozenset({"torch"}),
            canonical={
                "dependencies": [
                    {"name": "torch", "version": "2.13.0", "vulns": []},
                    {"name": "torchvision", "skip_reason": "missing"},
                ]
            },
            canonical_versions={"torch": "2.13.0", "torchvision": "0.28.0"},
        )


def test_accelerate_exception_is_version_bound() -> None:
    with pytest.raises(ValueError, match="revalidar"):
        audit_dependencies.validate_accelerate_mitigation({"accelerate": "1.15.0"})


def test_accelerate_guard_rejects_all_loader_aliases_before_opening_a_checkpoint(
    monkeypatch, tmp_path
) -> None:
    from parsezen.errors import ConversionError
    from parsezen.ocr_dependency_guard import protect_accelerate_loaders

    protect_accelerate_loaders()
    import accelerate
    from accelerate import big_modeling, utils
    from accelerate.utils import modeling
    from docling.document_converter import DocumentConverter

    assert DocumentConverter is not None
    index = tmp_path / "model.index.json"
    index.write_text(json.dumps({"weight_map": {"weight": "../private.bin"}}), encoding="utf-8")
    monkeypatch.setattr("builtins.open", lambda *args, **kwargs: pytest.fail("Opened unsafe model"))
    for loader in (
        accelerate.load_checkpoint_in_model,
        accelerate.load_checkpoint_and_dispatch,
        utils.load_checkpoint_in_model,
        modeling.load_checkpoint_in_model,
        big_modeling.load_checkpoint_in_model,
        big_modeling.load_checkpoint_and_dispatch,
    ):
        with pytest.raises(ConversionError, match="desactivado por seguridad"):
            loader(None, str(index))


def test_accelerate_audit_executes_the_production_mitigation() -> None:
    audit_dependencies.validate_accelerate_mitigation({"accelerate": "1.14.0"})
