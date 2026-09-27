"""Read-only, synthetic measurements for the maintenance audit."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from statistics import median
from tempfile import TemporaryDirectory
from time import perf_counter

AUDIT_ROOT = Path(__file__).resolve().parent


def _window_child() -> None:
    started = perf_counter()
    from PySide6.QtWidgets import QApplication

    from parsezen.presentation.main_window import ParsezenMainWindow
    from parsezen.settings import AppSettings

    imported = perf_counter()
    app = QApplication([])
    with TemporaryDirectory(prefix="ui-run-", dir=AUDIT_ROOT) as profile:
        root = Path(profile)
        window = ParsezenMainWindow(
            settings=AppSettings(),
            state_path=root / "workspace.sqlite3",
            history_path=root / "recent-jobs.json",
            work_checkpoint_root=root / "checkpoints",
            auto_discover_ai=False,
        )
        constructed = perf_counter()
        window.show()
        app.processEvents()
        shown = perf_counter()
        window.close()
        app.processEvents()
    print(
        json.dumps(
            {
                "import_seconds": round(imported - started, 3),
                "window_construction_seconds": round(constructed - imported, 3),
                "first_event_seconds": round(shown - constructed, 3),
                "total_seconds": round(shown - started, 3),
            }
        )
    )


def _window_measurement() -> dict[str, object]:
    runs: list[dict[str, float]] = []
    for _ in range(3):
        with TemporaryDirectory(prefix="env-run-", dir=AUDIT_ROOT) as profile:
            environment = dict(os.environ)
            environment["QT_QPA_PLATFORM"] = "offscreen"
            environment["APPDATA"] = profile
            environment["LOCALAPPDATA"] = profile
            environment["PYTHONPATH"] = str(AUDIT_ROOT.parent.parent / "src")
            completed = subprocess.run(
                [sys.executable, str(__file__), "window-child"],
                check=True,
                capture_output=True,
                text=True,
                env=environment,
                timeout=90,
            )
            runs.append(json.loads(completed.stdout))
    return {
        "method": "three fresh Python processes, Qt offscreen, synthetic profile, AI discovery off",
        "runs": runs,
        "median_total_seconds": round(median(run["total_seconds"] for run in runs), 3),
    }


def _model_measurement() -> dict[str, object]:
    from parsezen.direct_models import (
        active_direct_profile,
        direct_model_present,
        verified_direct_model_path,
    )
    from parsezen.local_ai_policy import ComponentCapability

    profile = active_direct_profile(ComponentCapability.TRANSLATION)
    assert profile is not None
    if not direct_model_present(profile.model_id):
        return {"status": "not-installed"}
    runs: list[float] = []
    for _ in range(3):
        started = perf_counter()
        verified_direct_model_path(profile.model_id)
        runs.append(round(perf_counter() - started, 3))
    return {
        "status": "verified",
        "model_size_bytes": profile.size_bytes,
        "method": "three consecutive SHA-256 checks, warm Windows file cache likely",
        "runs_seconds": runs,
        "median_seconds": round(median(runs), 3),
    }


if __name__ == "__main__":
    if sys.argv[1:] == ["window-child"]:
        _window_child()
    else:
        destination = AUDIT_ROOT / "runtime.json"
        if destination.exists():
            raise FileExistsError("Refusing to overwrite an earlier runtime measurement")
        result = {"window": _window_measurement(), "model": _model_measurement()}
        with destination.open("x", encoding="utf-8") as stream:
            stream.write(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps(result, ensure_ascii=False))
