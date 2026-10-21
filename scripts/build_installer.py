"""Construye el instalador de Windows de la app Tauri (F7).

Comprueba la huella de `pdfium.dll` (PDFium 7947 de pdfium-binaries, el mismo que trae
`pypdfium2` 5.12.1 en el entorno de desarrollo), lo prepara con sus licencias en
`target/instalador/` y ejecuta `tauri build` con el empaquetado NSIS activado solo para esta
orden: las compilaciones normales y la CI no lo necesitan. Los componentes grandes (lectura,
relectura, traducción) no van en el instalador: la app los descarga de su catálogo verificado.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PDFIUM_SHA256 = "cc2058add54b9da299a4496d526b79c375c6f0eb3a9000552a45c60f83765d55"
SITE_PACKAGES = ROOT / ".venv" / "Lib" / "site-packages"
DEFAULT_PDFIUM = SITE_PACKAGES / "pypdfium2_raw" / "pdfium.dll"
DEFAULT_LICENSES = SITE_PACKAGES / "pypdfium2-5.12.1.dist-info" / "licenses"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pdfium", type=Path, default=DEFAULT_PDFIUM)
    parser.add_argument("--licencias", type=Path, default=DEFAULT_LICENSES)
    args = parser.parse_args()

    if sha256(args.pdfium) != PDFIUM_SHA256:
        print("pdfium.dll no tiene la huella esperada; no se construye nada.", file=sys.stderr)
        return 1
    staging = ROOT / "target" / "instalador"
    shutil.rmtree(staging, ignore_errors=True)
    licenses = staging / "licencias" / "pdfium"
    licenses.mkdir(parents=True)
    shutil.copy2(args.pdfium, staging / "pdfium.dll")
    for license_file in sorted(args.licencias.rglob("*.txt")):
        shutil.copy2(license_file, licenses / license_file.name)
    shutil.copy2(ROOT / "LICENSE", staging / "licencias" / "LICENSE-Liblevo.txt")

    resources = {str(staging / "pdfium.dll"): "pdfium.dll"}
    for path in sorted((staging / "licencias").rglob("*")):
        if path.is_file():
            resources[str(path)] = path.relative_to(staging).as_posix()
    config = {
        "mainBinaryName": "Liblevo",
        "bundle": {
            "active": True,
            "targets": ["nsis"],
            "resources": resources,
            "publisher": "Jaime Acena",
            "shortDescription": "Convierte, revisa y traduce documentos en tu equipo",
            "windows": {"nsis": {"installMode": "currentUser", "languages": ["Spanish"]}},
        },
    }
    config_path = staging / "tauri.instalador.json"
    config_path.write_text(json.dumps(config, ensure_ascii=False, indent=2), encoding="utf-8")
    tauri = ROOT / "ui" / "node_modules" / ".bin" / "tauri.cmd"
    command = [str(tauri), "build", "--config", str(config_path)]
    result = subprocess.run(command, cwd=ROOT / "app", check=False)
    if result.returncode != 0:
        return result.returncode
    bundles = sorted((ROOT / "target" / "release" / "bundle" / "nsis").glob("*.exe"))
    for bundle in bundles:
        print(f"{bundle} {sha256(bundle)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
