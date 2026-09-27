"""Reject a local package build that cannot run the integrated Windows model."""

from __future__ import annotations

import sys
from importlib.metadata import PackageNotFoundError, version


def main() -> int:
    if sys.version_info[:2] != (3, 12):
        print("El paquete de Windows requiere Python 3.12.", file=sys.stderr)
        return 1

    for package, expected in (("llama-cpp-python", "0.3.35"), ("diskcache", "5.6.3")):
        try:
            installed = version(package)
        except PackageNotFoundError:
            print(f"Falta {package}; instala el lock Vulkan antes de empaquetar.", file=sys.stderr)
            return 1
        if installed != expected:
            print(
                f"{package} debe ser {expected} para este paquete; se encontró {installed}.",
                file=sys.stderr,
            )
            return 1

    try:
        version("pyinstaller")
        from llama_cpp import llama_cpp

        supports_gpu = llama_cpp.llama_supports_gpu_offload()
    except (ImportError, OSError, PackageNotFoundError, RuntimeError) as exc:
        print(f"No se puede cargar el motor de empaquetado: {type(exc).__name__}.", file=sys.stderr)
        return 1
    if not supports_gpu:
        print("El motor GGUF no incluye aceleración Vulkan.", file=sys.stderr)
        return 1

    print("Python 3.12 y motor GGUF integrado comprobados para el paquete local.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
