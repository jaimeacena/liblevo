"""Checks that local packaging refuses an incomplete integrated runtime."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import SimpleNamespace


def _runtime_module():
    path = Path(__file__).parents[1] / "distribution" / "windows" / "check_runtime.py"
    spec = importlib.util.spec_from_file_location("parsezen_package_runtime_check", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_package_preflight_rejects_missing_direct_model_binding(monkeypatch, capsys) -> None:
    runtime = _runtime_module()

    def installed_version(package: str) -> str:
        return "0.3.34" if package == "llama-cpp-python" else "5.6.3"

    monkeypatch.setattr(runtime, "version", installed_version)

    assert runtime.main() == 1
    assert "llama-cpp-python debe ser 0.3.35" in capsys.readouterr().err


def test_package_preflight_rejects_binding_without_gpu_support(monkeypatch, capsys) -> None:
    runtime = _runtime_module()
    versions = {"llama-cpp-python": "0.3.35", "diskcache": "5.6.3", "pyinstaller": "6.21.0"}
    monkeypatch.setattr(runtime, "version", versions.__getitem__)
    monkeypatch.setitem(
        sys.modules,
        "llama_cpp",
        SimpleNamespace(llama_cpp=SimpleNamespace(llama_supports_gpu_offload=lambda: False)),
    )

    assert runtime.main() == 1
    assert "no incluye aceleración Vulkan" in capsys.readouterr().err
