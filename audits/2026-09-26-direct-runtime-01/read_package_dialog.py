"""Read the text of our isolated package-smoke error dialog without clicking it."""

from __future__ import annotations

import sys

import win32gui
import win32process


def main() -> None:
    process_id = int(sys.argv[1])

    def top_window(handle: int, _extra: object) -> None:
        _thread, owner = win32process.GetWindowThreadProcessId(handle)
        if owner != process_id:
            return
        title = win32gui.GetWindowText(handle)
        if not title:
            return
        print("window", title)

        def child(child_handle: int, _unused: object) -> None:
            text = win32gui.GetWindowText(child_handle)
            if text:
                print("child", win32gui.GetClassName(child_handle), text[:2000])

        win32gui.EnumChildWindows(handle, child, None)

    win32gui.EnumWindows(top_window, None)


if __name__ == "__main__":
    main()
