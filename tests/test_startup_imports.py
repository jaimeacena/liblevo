from __future__ import annotations

import subprocess
import sys


def test_entrypoint_import_does_not_load_document_conversion_stacks() -> None:
    script = (
        "import sys; import liblevo.__main__; "
        "blocked={'liblevo.processing','markitdown','pptx','pdfplumber'}; "
        "loaded=blocked.intersection(sys.modules); "
        "raise SystemExit('unexpected imports: '+','.join(sorted(loaded)) if loaded else 0)"
    )

    completed = subprocess.run(
        [sys.executable, "-c", script],
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stderr
