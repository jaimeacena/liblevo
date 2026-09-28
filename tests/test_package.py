import hashlib
import json
from importlib import import_module, metadata
from pathlib import Path

from PySide6.QtGui import QImage

import liblevo


def test_package_exposes_version() -> None:
    assert liblevo.__version__ == metadata.version("liblevo")


def test_package_exposes_visible_and_storage_names() -> None:
    assert liblevo.APP_DISPLAY_NAME == "Liblevo"
    assert liblevo.APP_STORAGE_NAME == "Liblevo"


def test_ui_brand_asset_exists() -> None:
    branding = import_module("liblevo.branding")

    assert branding.BRAND_LOGO_PATH.is_file()
    assert branding.BRAND_DARK_LOGO_PATH.is_file()
    assert branding.APP_ICON_PATH.is_file()

    icon = QImage(str(branding.APP_ICON_PATH))
    assert not icon.isNull()
    assert (icon.width(), icon.height()) == (1024, 1024)
    assert icon.hasAlphaChannel()
    assert icon.pixelColor(0, 0).alpha() == 0
    assert branding.APP_ICON_ICO_PATH.is_file()


def test_windows_package_uses_liblevo_and_preserves_the_upgrade_identity() -> None:
    root = Path(__file__).parents[1]
    spec = (root / "distribution" / "windows" / "Liblevo.spec").read_text(encoding="utf-8")
    installer = (root / "distribution" / "windows" / "Liblevo.iss").read_text(encoding="utf-8")

    assert 'icon=str(root / "assets" / "branding" / "generated" / "liblevo-app-icon.ico")' in spec
    assert 'name="Liblevo"' in spec
    assert '#define AppName "Liblevo"' in installer
    assert "SetupIconFile=..\\..\\assets\\branding\\generated\\liblevo-app-icon.ico" in installer
    assert "OutputBaseFilename=Liblevo-Setup-{#AppVersion}" in installer
    assert 'Filename: "{app}\\{#AppName}.exe"' in installer
    assert "AppPublisher=Liblevo contributors" in installer
    assert "AppSupportURL=https://github.com/jaimeacena/liblevo/issues" in installer
    assert 'Type: filesandordirs; Name: "{app}\\_internal"' in installer
    assert "AppId={{B11F2D89-386D-42EF-9468-A8F69C329627}" in installer
    assert "UsePreviousAppDir=no" in installer
    assert "DefaultDirName={localappdata}\\Programs\\{#AppName}" in installer
    assert "{localappdata}\\Liblevo" not in installer


def test_windows_release_workflow_publishes_verifiable_artifacts() -> None:
    root = Path(__file__).parents[1]
    workflow = (root / ".github" / "workflows" / "package-windows.yml").read_text(encoding="utf-8")

    assert "actions/attest-build-provenance@v4" in workflow
    assert "Liblevo-Setup-*.exe.sha256" in workflow
    assert "python-environment.json" in workflow


def test_all_brand_assets_match_the_current_manifest() -> None:
    root = Path(__file__).parents[1]
    generated = root / "assets" / "branding" / "generated"
    manifest = json.loads((generated / "manifest.json").read_text(encoding="utf-8"))

    assert manifest["brand"] == "Liblevo"
    for key in ("source", "font"):
        assert (
            hashlib.sha256((root / manifest[key]).read_bytes()).hexdigest()
            == manifest[key + "_sha256"]
        )
    for name, expected in manifest["outputs"].items():
        assert hashlib.sha256((generated / name).read_bytes()).hexdigest() == expected["sha256"]
    assert {path.name for path in generated.iterdir() if path.is_file()} == {
        "manifest.json",
        *manifest["outputs"],
    }


def test_current_profile_reopens_existing_settings_and_model_storage(
    monkeypatch, tmp_path: Path
) -> None:
    settings = import_module("liblevo.settings")
    models = import_module("liblevo.direct_models")
    recent = import_module("liblevo.recent_activity")

    def profile_path(name: str, *, appauthor: bool) -> Path:
        assert appauthor is False
        return tmp_path / name

    monkeypatch.setattr(settings, "user_config_path", profile_path)
    monkeypatch.setattr(models, "user_data_path", profile_path)
    monkeypatch.setattr(recent, "user_data_path", profile_path)
    profile = tmp_path / "Liblevo"
    profile.mkdir()
    expected = settings.AppSettings(output_directory=tmp_path / "resultados")
    settings.save_settings(expected, profile / "settings.json")
    (profile / "models").mkdir()
    marker = profile / "models" / "existing-model.marker"
    marker.write_bytes(b"existing local model")

    assert settings.load_settings() == expected
    assert settings.get_settings_path() == profile / "settings.json"
    assert models.direct_model_root() == profile / "models"
    assert marker.read_bytes() == b"existing local model"
    assert recent.get_history_path() == profile / "recent-jobs.json"


def test_desktop_entry_point_imports() -> None:
    entry_point = import_module("liblevo.__main__")

    assert callable(entry_point.main)
    assert callable(entry_point.build_main_window)


def test_ui_module_imports() -> None:
    main_window = import_module("liblevo.presentation.main_window")
    local_ai = import_module("liblevo.presentation.local_ai_controller")
    runner = import_module("liblevo.presentation.processing_runner")

    assert main_window.LiblevoMainWindow is not None
    assert local_ai.LocalAIController is not None
    assert runner.ProcessingWorker is not None


def test_desktop_package_smoke_mode_starts_and_closes_offscreen(
    monkeypatch,
) -> None:
    monkeypatch.setenv("QT_QPA_PLATFORM", "offscreen")
    entry_point = import_module("liblevo.__main__")

    assert entry_point.main(["liblevo", "--package-smoke"]) == 0


def test_ai_and_settings_modules_import() -> None:
    improvement = import_module("liblevo.improvement")
    settings = import_module("liblevo.settings")

    assert improvement.ImprovementMode.CLEAN.value == "clean"
    assert settings.AppSettings().context_window is None
