"""Read-only contrast spot check of current desktop color tokens."""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "src"))

from parsezen.presentation.design_system import _DARK_COLORS, _LIGHT_COLORS  # noqa: E402


def luminance(value: str) -> float:
    rgb = [int(value[index : index + 2], 16) / 255 for index in (1, 3, 5)]
    linear = [part / 12.92 if part <= 0.04045 else ((part + 0.055) / 1.055) ** 2.4 for part in rgb]
    return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]


def ratio(foreground: str, background: str) -> float:
    lighter, darker = sorted((luminance(foreground), luminance(background)), reverse=True)
    return round((lighter + 0.05) / (darker + 0.05), 2)


def main() -> None:
    combinations = (
        ("text_primary", "surface"),
        ("text_secondary", "surface"),
        ("text_muted", "surface"),
        ("text_inverse", "action_primary"),
        ("border_focus", "surface"),
    )
    print(
        json.dumps(
            {
                theme: {
                    f"{foreground}/{background}": ratio(
                        getattr(colors, foreground), getattr(colors, background)
                    )
                    for foreground, background in combinations
                }
                for theme, colors in (("light", _LIGHT_COLORS), ("dark", _DARK_COLORS))
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
