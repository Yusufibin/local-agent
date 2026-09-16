#!/usr/bin/env python3
"""Decode scripts/tauri-icons.tar.b64 into src-tauri/icons/. ASCII payload only — no secrets."""
from __future__ import annotations

import base64
import io
import os
import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ARCHIVE = ROOT / "scripts" / "tauri-icons.tar.b64"
PARTS_DIR = ROOT / "scripts" / "tauri-icons.b64"
DEST = ROOT / "src-tauri" / "icons"
ALLOWED_EXT = {".png", ".ico", ".icns"}


def load_b64() -> str:
    parts = sorted(PARTS_DIR.glob("*.txt")) if PARTS_DIR.is_dir() else []
    if parts:
        return "".join(p.read_text(encoding="ascii") for p in parts)
    if ARCHIVE.is_file():
        return ARCHIVE.read_text(encoding="ascii")
    raise FileNotFoundError(f"missing {PARTS_DIR}/*.txt and {ARCHIVE}")


def main() -> int:
    try:
        b64 = load_b64()
    except FileNotFoundError as exc:
        print(exc, file=sys.stderr)
        return 1
    data = base64.b64decode("".join(b64.split()), validate=False)
    DEST.mkdir(parents=True, exist_ok=True)
    written = []
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tf:
        for member in tf.getmembers():
            if not member.isfile():
                continue
            name = os.path.basename(member.name)
            ext = os.path.splitext(name)[1].lower()
            if not name or name.startswith(".") or ext not in ALLOWED_EXT:
                continue
            src = tf.extractfile(member)
            if src is None:
                continue
            payload = src.read()
            (DEST / name).write_bytes(payload)
            written.append(f"{name}:{len(payload)}")
    if "icon.ico" not in {w.split(":", 1)[0] for w in written}:
        print("icon.ico missing after materialize", file=sys.stderr)
        return 1
    print("materialized", *sorted(written))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
