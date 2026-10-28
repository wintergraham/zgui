#!/usr/bin/env python3
"""What a window shows after frames that each draw their damage alone, read
off the screen: `present_check` moves a mark along a row over 30 frames,
then holds its last. Every place the mark passed must show the background
again, and the last the mark: a buffer brought up to date wrongly (Windows'
swap chain of the renderer's own, dxgi_present.rs)
leaves a stale mark or a hole.

`--lose` runs each path a second time losing the device after the frames:
a new renderer for the same window draws the last frame whole.

Its presents' dirty rectangles let the compositor keep the rest from its
own copy, which hides a wrong buffer; `own-whole` presents each buffer whole
(`ZGX_DXGI_PRESENT_WHOLE`), so every buffer must be complete. Runs every
path (`own`, `own-whole`, wgpu's) unless told one.

    cargo build --release -p zgx-gpu --example present_check
    python scripts/windows_present_probe.py target/release/examples/present_check.exe [--only own|own-whole|wgpu]
"""
import argparse
import ctypes
import os
import pathlib
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import windows_compare as wc  # noqa: E402
import windows_input as win  # noqa: E402

BACKGROUND = (20, 40, 80)
MARK = (220, 30, 30)
STEPS = 30


PATHS = {"own": {},
         "own-whole": {"ZGX_DXGI_PRESENT_WHOLE": "1"},
         "wgpu": {"ZGX_DXGI_PRESENT": "0"}}


def check(exe, path, lose):
    env = dict(os.environ)
    for key in ("ZGX_DXGI_PRESENT", "ZGX_DXGI_PRESENT_WHOLE"):
        env.pop(key, None)
    env.update(PATHS[path])
    proc = subprocess.Popen([str(exe)] + (["--lose"] if lose else []), env=env, stdout=subprocess.PIPE, text=True)
    try:
        if proc.stdout.readline().strip() != "ready":
            return ["no ready line"]
        hwnd = wc.bring_forward(proc)
        if hwnd is None:
            return ["the window isn't in front and uncovered"]
        time.sleep(0.5)
        shots = [("as presented", win.capture(hwnd))]
        # Minimized and shown again, the compositor reads the window's whole
        # buffer, not just the regions its last presents marked changed.
        ctypes.windll.user32.ShowWindow(ctypes.c_void_p(hwnd), 6)  # SW_MINIMIZE
        time.sleep(0.5)
        ctypes.windll.user32.ShowWindow(ctypes.c_void_p(hwnd), 9)  # SW_RESTORE
        if wc.bring_forward(proc) is None:
            return ["the window isn't in front and uncovered after restoring"]
        time.sleep(0.5)
        shots.append(("restored", win.capture(hwnd)))
        failures = []
        for name, shot in shots:
            failures += [f"{name}: {failure}" for failure in marks(shot)]
        return failures
    finally:
        proc.terminate()
        proc.wait(timeout=10)


def marks(shot):
    """What differs from the expected row of marks in `shot`."""
    failures = []
    for step in range(STEPS + 1):
        x, y = 10 + step * 12 + 5, 50
        want = MARK if step == STEPS else BACKGROUND
        got = win.pixel(shot, x, y)
        if max(abs(a - b) for a, b in zip(got, want)) > 2:
            failures.append(f"step {step} at ({x}, {y}): {got}, not {want}")
    # Between the marks and around them: background.
    for x, y in [(5, 5), (200, 20), (395, 95), (16, 50), (395, 50)]:
        got = win.pixel(shot, x, y)
        if max(abs(a - b) for a, b in zip(got, BACKGROUND)) > 2:
            failures.append(f"background at ({x}, {y}): {got}")
    return failures


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("exe")
    parser.add_argument("--only", choices=list(PATHS))
    parser.add_argument("--lose", action="store_true", help="also lose the device after the frames")
    args = parser.parse_args()
    failed = False
    for name, lose in [(name, lose) for name in PATHS for lose in ([False, True] if args.lose else [False])]:
        if args.only and args.only != name:
            continue
        failures = check(args.exe, name, lose)
        name = f"{name}, device lost" if lose else name
        print(f"{name}: {'ok' if not failures else 'FAILED'}")
        for failure in failures:
            print(f"  {failure}")
        failed |= bool(failures)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
