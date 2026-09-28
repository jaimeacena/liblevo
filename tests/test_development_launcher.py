from pathlib import Path


def test_development_launcher_always_prefers_current_checkout_sources() -> None:
    launcher = Path("Abrir Liblevo.cmd").read_text(encoding="utf-8")

    assert 'set "PYTHONPATH=%~dp0src;%PYTHONPATH%"' in launcher
    assert '"%LIBLEVO_PYTHON%" -m liblevo' in launcher


def test_current_launchers_use_their_own_folder_and_local_runtime() -> None:
    for action in ("Abrir", "Validar"):
        launcher = Path(f"{action} Liblevo.cmd").read_text(encoding="utf-8")
        assert 'cd /d "%~dp0"' in launcher
        assert "LIBLEVO_PYTHON=%~dp0.venv\\Scripts\\python" in launcher
    validation = Path("Validar Liblevo.cmd").read_text(encoding="utf-8")
    assert '"%LIBLEVO_PYTHON%" -m pytest -m acceptance' in validation
