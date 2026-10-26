#!/usr/bin/env python3
"""Measure what an example costs at rest on Windows: CPU cycles and time while
its window sits untouched, then while the pointer rests over it.

    cargo build --release -p zgx-desktop --example gallery
    python scripts/windows_idle_probe.py target/release/examples/gallery.exe

Cycles (QueryProcessCycleTime) resolve what CPU time (a 15.6 ms scheduler
tick) cannot; a window at rest should spend close to none. The window is
brought to the front, as a user has it (a window opened from a process in
the background stays behind the foreground one), with Windows' High
performance plan held and Zeron minimized (both restored after), as the
comparison runner measures.

At rest the window must present nothing (`ZGX_PRESENT_LOG`, counted while
it rests; `--max-presents`). Each thread's cycles are listed, and summed
apart for zgx's own threads and the graphics driver's: on NVIDIA, Direct3D
12 keeps two driver threads busy at rest (about 1 M cycles a second),
where zgx's own spend none.
"""
import argparse
import os
import pathlib
import subprocess
import tempfile
import time

import windows_compare
import windows_input as win

# Modules whose threads are the graphics driver's or its runtime's, not zgx's.
DRIVER = ("nvwgf2um", "nvoglv", "amdxx", "atidxx", "igd", "igc", "d3d12", "d3d11", "dxgi", "dxcore")


def presents(log):
    """The presentation events logged so far."""
    return sum(1 for line in log.read_text(errors="replace").splitlines() if line.startswith("PRESENT "))


def window_cost(pid, seconds, log):
    """Cycles, CPU time, presents, and zgx's own threads' cycles, over
    `seconds`; each thread's cycles printed."""
    cycles, cpu, before, shown = win.cycles(pid), win.cpu_seconds(pid), win.thread_cycles(pid), presents(log)
    time.sleep(seconds)
    after = win.thread_cycles(pid)
    busy = sorted(((after[k] - before.get(k, 0), k) for k in after), reverse=True)
    own = 0
    for spent, (tid, name) in busy:
        driver = any(module in name.lower() for module in DRIVER)
        own += 0 if driver else spent
        if spent:
            print(f"    thread {tid} {name or '(unnamed)'}{' (driver)' if driver else ''}: "
                  f"{spent / seconds / 1e6:.2f} M cycles/s")
    return win.cycles(pid) - cycles, win.cpu_seconds(pid) - cpu, presents(log) - shown, own


def measure(proc, args, log):
    window = windows_compare.bring_forward(proc)
    if not window:
        raise AssertionError("the window stayed covered")
    time.sleep(args.settle)
    print("at rest, by thread:")
    rest = window_cost(proc.pid, args.seconds, log)
    width, height = win.client_size(window)
    win.move(window, width // 2, height // 2)
    time.sleep(args.settle)
    print("hovered, by thread:")
    hover = window_cost(proc.pid, args.seconds, log)
    private, working = win.memory(proc.pid)
    print(f"memory: {private / 2**20:.1f} MiB private, {working / 2**20:.1f} MiB working set")
    for name, (cycles, cpu, shown, own) in [("at rest", rest), ("pointer resting over it", hover)]:
        print(f"{name}: {cycles / args.seconds / 1e6:.2f} M cycles/s ({own / args.seconds / 1e6:.2f} M zgx's own), "
              f"{cpu / args.seconds * 1000:.2f} ms CPU/s, {shown} presents")
    if rest[2] > args.max_presents:
        raise AssertionError(f"at rest: {rest[2]} presents, over {args.max_presents}")
    if args.max_own_cycles_per_second is not None and rest[3] / args.seconds > args.max_own_cycles_per_second:
        raise AssertionError(f"at rest: zgx's own threads {rest[3] / args.seconds:.0f} cycles/s over the limit")
    if args.max_cycles_per_second is not None:
        for name, (cycles, _, _, _) in [("at rest", rest), ("hovered", hover)]:
            if cycles / args.seconds > args.max_cycles_per_second:
                raise AssertionError(f"{name}: {cycles / args.seconds:.0f} cycles/s over the limit")
    win.close(window)
    proc.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=pathlib.Path)
    parser.add_argument("--seconds", type=float, default=10)
    parser.add_argument("--settle", type=float, default=3, help="seconds to wait after the window shows")
    parser.add_argument("--env", action="append", default=[], metavar="NAME=VALUE",
                        help="set in the app's environment (repeatable)")
    parser.add_argument("--max-cycles-per-second", type=float,
                        help="fail when the window at rest uses more than this")
    parser.add_argument("--max-own-cycles-per-second", type=float,
                        help="fail when zgx's own threads (not the graphics driver's) use more than this at rest")
    parser.add_argument("--max-presents", type=int, default=0,
                        help="presents allowed while the window rests (default none)")
    args = parser.parse_args()
    env = dict(os.environ, ZGX_PRESENT_LOG="1", **dict(pair.split("=", 1) for pair in args.env))
    handle, name = tempfile.mkstemp(prefix="zgx-idle-", suffix=".log")
    os.close(handle)
    log = pathlib.Path(name)
    try:
        with windows_compare.PowerPlan(windows_compare.HIGH_PERFORMANCE), windows_compare.Minimized(["Zeron"]), \
                open(log, "w") as output:
            proc = subprocess.Popen([str(args.binary.resolve())], env=env, stdout=output, stderr=output)
            try:
                measure(proc, args, log)
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
    finally:
        log.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
