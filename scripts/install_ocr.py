#!/usr/bin/env python3
"""
Minimal OCR installer script for Tauri resource packaging.

This script keeps the installer command chain available on repositories where
the full Windows-oriented OCR installer was not committed yet.
"""

from __future__ import annotations

import json
import sys


def log(message: str) -> None:
    print(message, flush=True)


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else "install"

    if mode == "uninstall":
        log("OCR runtime uninstall placeholder executed")
        print(json.dumps({"success": True, "action": "uninstall"}, ensure_ascii=False))
        return 0

    log("OCR runtime install placeholder executed")
    print(json.dumps({"success": True, "action": "install"}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
