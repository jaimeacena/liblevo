"""Open Parsezen with only synthetic, audit-owned state and outputs."""

from __future__ import annotations

import os
import sys
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[2]
AUDIT_ROOT = Path(__file__).resolve().parent
if "--check-reopen" in sys.argv:
    os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.path.insert(0, str(REPOSITORY / "src"))

from PySide6.QtCore import QSettings  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from parsezen.presentation.design_system import (  # noqa: E402
    apply_parsezen_theme,
    preferred_theme_mode,
)
from parsezen.presentation.main_window import ParsezenMainWindow  # noqa: E402
from parsezen.settings import AppSettings, save_settings  # noqa: E402


def main() -> int:
    with_sample = "--synthetic" in sys.argv
    profile = AUDIT_ROOT / ("ui-sample-profile" if with_sample else "ui-profile")
    profile.mkdir(exist_ok=True)
    QSettings.setDefaultFormat(QSettings.Format.IniFormat)
    QSettings.setPath(QSettings.Format.IniFormat, QSettings.Scope.UserScope, str(profile))
    application = QApplication(["parsezen-isolated-audit"])
    apply_parsezen_theme(application, preferred_theme_mode())
    window = ParsezenMainWindow(
        settings=AppSettings(output_directory=profile / "outputs"),
        on_settings_changed=lambda value: save_settings(value, profile / "settings.json"),
        auto_discover_ai=False,
        history_path=profile / "history.json",
        work_checkpoint_root=profile / "work-checkpoints",
        state_path=profile / "workspace.sqlite3",
    )
    if with_sample and not window._job_queue.jobs:
        window.set_source_paths((AUDIT_ROOT / "synthetic-20-pages.pdf",))
    if "--check-reopen" in sys.argv:
        jobs = window._job_queue.jobs
        assert len(jobs) == 1 and jobs[0].source.path.name == "synthetic-20-pages.pdf"
        print({"case": "UI-RECOVER-01", "jobs_recovered": len(jobs), "result": "PASS"})
        window.close()
        application.processEvents()
        return 0
    window.setWindowTitle("Parsezen - auditoría aislada")
    window.show()
    return int(application.exec())


if __name__ == "__main__":
    raise SystemExit(main())
