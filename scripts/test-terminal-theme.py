#!/usr/bin/env python3
"""macOS native rendering smoke test; run after cargo build (opens a temporary window)."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
if sys.platform != "darwin":
    raise SystemExit("This native rendering test currently requires macOS")
libraries = list((ROOT / "target/debug/build").glob("gpui-libghostty-*/out/libghostty-internal.a"))
if not libraries:
    raise SystemExit("Run cargo build --locked first")
library = max(libraries, key=lambda p: p.stat().st_mtime)
frameworks = ["AppKit", "Carbon", "CoreFoundation", "CoreGraphics", "CoreText", "CoreVideo",
              "Foundation", "IOSurface", "Metal", "QuartzCore"]
with tempfile.TemporaryDirectory(prefix="zvim-theme-test-") as temporary:
    directory = Path(temporary)
    baseline = directory / "baseline"
    overlay = directory / "overlay"
    baseline.write_text("background = #112233\nforeground = #eeeeee\nfont-family = Menlo\n")
    overlay.write_text("background = #334455\nforeground = #ddeeff\npalette = 1=#abcdef\n")
    binary = directory / "test"
    subprocess.run(["xcrun", "clang", "-fblocks", "-fno-objc-arc", "-I",
                    str(ROOT / "crates/zvim-ghostty/include"), str(ROOT / "tests/native/terminal_theme.m"),
                    str(library), "-lc++", "-lobjc", *[x for f in frameworks for x in ("-framework", f)],
                    "-o", str(binary)], check=True)
    environment = os.environ.copy()
    environment["ZDOTDIR"] = str(directory)
    environment["GHOSTTY_RESOURCES_DIR"] = str(ROOT / "target/ghostty-resources/ghostty")
    subprocess.run([str(binary), str(baseline), str(overlay)], env=environment, check=True, timeout=20)
