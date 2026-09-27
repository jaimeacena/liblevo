"""Run pip-audit while rejecting dependencies that silently escape coverage."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

from packaging.utils import canonicalize_name

_LOCKED_REQUIREMENT = re.compile(r"^([A-Za-z0-9_.-]+)==([^\s\\]+)")


def locked_versions(path: Path) -> dict[str, str]:
    """Return canonical package names and exact versions from a compiled lockfile."""

    versions: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        match = _LOCKED_REQUIREMENT.match(line.strip())
        if match is not None:
            name = canonicalize_name(match.group(1))
            if name in versions:
                raise ValueError(f"Dependencia duplicada en el lock: {name}.")
            versions[name] = match.group(2)
    if not versions:
        raise ValueError("El lock no contiene versiones auditables.")
    return versions


def _audit_inventory(report: dict[str, Any], versions: dict[str, str]) -> dict[str, dict[str, Any]]:
    dependencies = report.get("dependencies")
    if not isinstance(dependencies, list) or not versions:
        raise ValueError("pip-audit no devolvió una lista de dependencias válida.")
    results: dict[str, dict[str, Any]] = {}
    for item in dependencies:
        if not isinstance(item, dict) or not isinstance(item.get("name"), str) or not item["name"]:
            raise ValueError("pip-audit devolvió una dependencia mal formada.")
        name = canonicalize_name(item["name"])
        if name in results:
            raise ValueError(f"Dependencia duplicada en la auditoría: {name}.")
        results[name] = item
    missing = sorted(versions.keys() - results.keys())
    unexpected = sorted(results.keys() - versions.keys())
    if missing or unexpected:
        raise ValueError(
            f"Inventario distinto del lock; faltan: {', '.join(missing) or 'ninguna'}; "
            f"sobran: {', '.join(unexpected) or 'ninguna'}."
        )
    for name, item in results.items():
        if item.get("version") is not None and item["version"] != versions[name]:
            raise ValueError(f"pip-audit no examinó la versión esperada de {name}.")
        if item.get("skip_reason"):
            if not isinstance(item["skip_reason"], str):
                raise ValueError(f"Omisión mal formada: {name}.")
            continue
        if item.get("version") != versions[name]:
            raise ValueError(f"pip-audit no examinó la versión esperada de {name}.")
        if not isinstance(item.get("vulns"), list):
            raise ValueError(f"Resultado de vulnerabilidades incompleto: {name}.")
    return results


def validate_lock_alignment(primary: dict[str, str], canonical: dict[str, str]) -> None:
    """The Windows lock may only differ by the two approved CPU build suffixes."""
    if primary.keys() != canonical.keys():
        raise ValueError("Los locks no contienen las mismas dependencias.")
    for name, version in primary.items():
        expected = canonical[name]
        if version == expected or (
            name in {"torch", "torchvision"} and version == expected + "+cpu"
        ):
            continue
        raise ValueError(
            f"La variante {name} {version} no coincide con la versión canónica {expected}."
        )


def validate_audit_coverage(
    primary: dict[str, Any],
    *,
    primary_versions: dict[str, str],
    allowed_variant_skips: frozenset[str] = frozenset(),
    canonical: dict[str, Any] | None = None,
    canonical_versions: dict[str, str] | None = None,
) -> None:
    """Reject unexpected skips and prove allowed variants through canonical packages."""

    if (canonical is None) != (canonical_versions is None):
        raise ValueError("Falta el informe o inventario de la auditoría canónica.")
    results = _audit_inventory(primary, primary_versions)
    canonical_results = (
        _audit_inventory(canonical, canonical_versions)
        if canonical is not None and canonical_versions is not None
        else {}
    )
    canonical_skips = sorted(
        name for name, item in canonical_results.items() if item.get("skip_reason")
    )
    if canonical_skips:
        raise ValueError(
            "omisiones inesperadas en la auditoría canónica: " + ", ".join(canonical_skips)
        )
    skipped = {name for name, item in results.items() if item.get("skip_reason")}
    allowed = {canonicalize_name(name) for name in allowed_variant_skips}
    unexpected = sorted(skipped - allowed)
    if unexpected:
        raise ValueError(f"omisiones inesperadas: {', '.join(unexpected)}")
    if not skipped:
        return
    if canonical is None or canonical_versions is None:
        raise ValueError("Falta la auditoría canónica para justificar variantes omitidas.")
    for name in sorted(skipped):
        primary_version = primary_versions.get(name)
        canonical_version = canonical_versions.get(name)
        result = canonical_results.get(name)
        if primary_version is None or canonical_version is None or result is None:
            raise ValueError(f"No se pudo vincular {name} con su paquete canónico auditado.")
        if result.get("skip_reason"):
            raise ValueError(f"La variante canónica de {name} también fue omitida.")
        if result.get("version") != canonical_version:
            raise ValueError(f"pip-audit no examinó la versión canónica esperada de {name}.")
        if primary_version.split("+", 1)[0] != canonical_version:
            raise ValueError(
                f"La variante {name} {primary_version} no coincide con "
                f"la versión canónica {canonical_version}."
            )


def run_audit(path: Path, ignored_vulnerabilities: tuple[str, ...]) -> dict[str, Any]:
    """Run pip-audit with machine-readable output and return its report."""

    command = [
        sys.executable,
        "-m",
        "pip_audit",
        "-r",
        str(path),
        "--no-deps",
        "--disable-pip",
        "--format=json",
    ]
    for vulnerability in ignored_vulnerabilities:
        command.extend(("--ignore-vuln", vulnerability))
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    try:
        payload = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError("pip-audit no devolvió un informe JSON válido.") from exc
    if completed.returncode != 0:
        findings: list[str] = []
        dependencies = payload.get("dependencies", []) if isinstance(payload, dict) else []
        for item in dependencies if isinstance(dependencies, list) else []:
            if not isinstance(item, dict):
                continue
            name, version = item.get("name"), item.get("version")
            if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9_.-]{1,128}", name):
                continue
            if not isinstance(version, str) or not re.fullmatch(r"[A-Za-z0-9_.+!-]{1,80}", version):
                continue
            vulnerabilities = item.get("vulns", [])
            for vulnerability in vulnerabilities if isinstance(vulnerabilities, list) else []:
                identifier = vulnerability.get("id") if isinstance(vulnerability, dict) else None
                if isinstance(identifier, str) and re.fullmatch(r"[A-Za-z0-9-]{1,80}", identifier):
                    findings.append(f"{name} {version}: {identifier}")
        details = " " + "; ".join(findings[:20]) if findings else ""
        raise RuntimeError(
            "pip-audit detectó una vulnerabilidad o no pudo completar la auditoría." + details
        )
    if not isinstance(payload, dict):
        raise RuntimeError("pip-audit devolvió un informe inesperado.")
    return payload


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("lockfile", type=Path)
    parser.add_argument("--canonical-lock", type=Path)
    parser.add_argument("--allow-variant-skip", action="append", default=[])
    parser.add_argument("--ignore-vuln", action="append", default=[])
    parser.add_argument("--allow-blocked-accelerate", action="store_true")
    args = parser.parse_args()
    try:
        primary_versions = locked_versions(args.lockfile)
        canonical_versions = (
            locked_versions(args.canonical_lock) if args.canonical_lock is not None else None
        )
        if canonical_versions is not None:
            validate_lock_alignment(primary_versions, canonical_versions)
        if args.allow_blocked_accelerate:
            validate_accelerate_mitigation(primary_versions)
            print("Excepción comprobada: Accelerate 1.14.0, cargadores vulnerables desactivados.")
            args.ignore_vuln.append("PYSEC-2026-3804")
        primary = run_audit(args.lockfile, tuple(args.ignore_vuln))
        canonical = (
            run_audit(args.canonical_lock, tuple(args.ignore_vuln))
            if args.canonical_lock is not None
            else None
        )
        validate_audit_coverage(
            primary,
            primary_versions=primary_versions,
            allowed_variant_skips=frozenset(args.allow_variant_skip),
            canonical=canonical,
            canonical_versions=canonical_versions,
        )
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"Auditoría incompleta: {exc}")
        return 1
    print("Auditoría completa: no hay dependencias sin cobertura inesperada.")
    return 0


def validate_accelerate_mitigation(versions: dict[str, str]) -> None:
    """Permit only the pinned advisory after executing the production fail-closed guard."""
    from importlib.metadata import version

    if versions.get("accelerate") != "1.14.0" or version("accelerate") != "1.14.0":
        raise ValueError("La excepción de Accelerate requiere revalidar esta versión.")
    from parsezen.errors import ConversionError
    from parsezen.ocr_dependency_guard import protect_accelerate_loaders

    protect_accelerate_loaders()
    import accelerate
    from accelerate import big_modeling, utils
    from accelerate.utils import modeling

    for loader in (
        accelerate.load_checkpoint_in_model,
        accelerate.load_checkpoint_and_dispatch,
        utils.load_checkpoint_in_model,
        modeling.load_checkpoint_in_model,
        big_modeling.load_checkpoint_in_model,
        big_modeling.load_checkpoint_and_dispatch,
    ):
        try:
            loader(None, "../unsupported.index.json")
        except ConversionError:
            continue
        raise RuntimeError("La protección de Accelerate no rechazó el cargador inseguro.")


if __name__ == "__main__":
    raise SystemExit(main())
