#!/usr/bin/env python3
"""Build the external Turbo Bar and customize its DMG volume icon."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DEFAULT_TURBO_DIR = ROOT
DEFAULT_VENV = Path("/Users/lpa/ComfyUI-venv")


def run(command: list[str], *, cwd: Path, env: dict[str, str]) -> None:
    print("+", " ".join(command))
    subprocess.run(command, cwd=cwd, env=env, check=True)


def replace_dmg_volume_icon(dmg_path: Path, icon_path: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="comfypanel-dmg-") as temp_dir:
        temp_root = Path(temp_dir)
        read_write_dmg = temp_root / "rw.dmg"
        rebuilt_dmg = temp_root / "rebuilt.dmg"
        mount_point = temp_root / "mount"
        mount_point.mkdir()

        subprocess.run(
            ["hdiutil", "convert", str(dmg_path), "-format", "UDRW", "-o", str(read_write_dmg)],
            check=True,
        )
        subprocess.run(
            [
                "hdiutil",
                "attach",
                "-nobrowse",
                "-noautoopen",
                "-readwrite",
                "-mountpoint",
                str(mount_point),
                str(read_write_dmg),
            ],
            check=True,
        )
        try:
            shutil.copy2(icon_path, mount_point / ".VolumeIcon.icns")
            subprocess.run(["SetFile", "-a", "C", str(mount_point)], check=True)
            subprocess.run(["SetFile", "-a", "V", str(mount_point / ".VolumeIcon.icns")], check=True)
        finally:
            subprocess.run(["hdiutil", "detach", str(mount_point)], check=True)

        subprocess.run(
            [
                "hdiutil",
                "convert",
                str(read_write_dmg),
                "-format",
                "UDZO",
                "-imagekey",
                "zlib-level=9",
                "-o",
                str(rebuilt_dmg),
            ],
            check=True,
        )
        shutil.move(rebuilt_dmg, dmg_path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--turbo-dir", type=Path, default=None)
    parser.add_argument("--venv", type=Path, default=DEFAULT_VENV)
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    turbo_dir = (args.turbo_dir or Path(os.environ.get("COMFYPANEL_TURBO_BAR_DIR", DEFAULT_TURBO_DIR))).expanduser().resolve()

    if not turbo_dir.is_dir():
        raise SystemExit(f"Turbo Bar directory not found: {turbo_dir}")

    # Locate tauri CLI: local node_modules > npx
    local_tauri = turbo_dir / "node_modules/.bin/tauri"
    if local_tauri.is_file():
        tauri_cmd = [str(local_tauri)]
    else:
        tauri_cmd = ["npx", "tauri"]

    env = os.environ.copy()
    home = Path.home()
    cargo_bin = home / ".cargo/bin"
    env["PATH"] = ":".join(filter(None, [
        str(cargo_bin) if cargo_bin.is_dir() else "",
        "/usr/local/bin",
        "/opt/homebrew/bin",
        "/usr/bin",
        "/bin",
        env.get("PATH", ""),
    ]))

    if not args.skip_build:
        run([*tauri_cmd, "build"], cwd=turbo_dir, env=env)

    dmgs = sorted(
        turbo_dir.glob("src-tauri/target/**/release/bundle/dmg/*.dmg"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not dmgs:
        raise SystemExit(f"No DMG found in: {turbo_dir / 'src-tauri/target'}")

    volume_icon = turbo_dir / "src-tauri/icons/volume.icns"
    if not volume_icon.is_file():
        raise SystemExit(f"DMG volume icon not found: {volume_icon}")

    replace_dmg_volume_icon(dmgs[0], volume_icon)
    print(f"Turbo Bar package: {dmgs[0]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
