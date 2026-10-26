#!/usr/bin/env python3
"""Whether content animating at the display's full rate gets it on macOS.

    python3 scripts/macos_full_rate_probe.py [--target-dir DIR] [--launches N]

Builds the animation_workload example (release) and runs it a few times for
four seconds each; each run reports the frames it animated. It fails when a
run's rate is under 90% of the display's maximum refresh (NSScreen's
maximumFramesPerSecond): a window pacing full-rate frames at half rate (a
120 Hz panel whose display mode reads 60 Hz) shows as about 60 fps. Several
launches, because the pacing is decided as each window starts.

The app is launched as a direct child, from a temporary directory, with a
clean environment, and sends no input: safe on a desktop in use. If a system
dialog appears, stop and report it; never click it. macOS only.
"""
import argparse
import os
import pathlib
import platform
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
SCREEN = 'import AppKit\nprint(NSScreen.screens.map(\\.maximumFramesPerSecond).max() ?? 60)\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--target-dir', default=os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target')))
    parser.add_argument('--launches', type=int, default=3)
    args = parser.parse_args()
    if platform.system() != 'Darwin':
        print('macOS only')
        return 2
    env = dict(os.environ, CARGO_TARGET_DIR=args.target_dir)
    subprocess.run(['cargo', 'build', '-q', '--release', '-p', 'zgx-desktop', '--example', 'animation_workload', '--locked'],
                   cwd=ROOT, env=env, check=True)
    binary = pathlib.Path(args.target_dir) / 'release/examples/animation_workload'
    clean = {key: os.environ[key] for key in ('HOME', 'USER') if key in os.environ}
    clean['PATH'] = '/usr/bin:/bin'
    with tempfile.TemporaryDirectory(prefix='zgx-full-rate-') as cwd:
        screen = pathlib.Path(cwd) / 'screen.swift'
        screen.write_text(SCREEN)
        display = int(subprocess.run(['swift', str(screen)], capture_output=True, text=True, check=True).stdout.strip())
        print(f'display maximum refresh: {display} Hz')
        rates = []
        for launch in range(args.launches):
            run = subprocess.run([str(binary)], cwd=cwd, env=dict(clean, ZGX_MODE='animate', ZGX_SECONDS='4'),
                                 capture_output=True, text=True, timeout=60)
            found = re.search(r'fps=([\d.]+)', run.stdout + run.stderr)
            rate = float(found.group(1)) if found else 0.
            rates.append(rate)
            print(f"{'ok  ' if rate >= 0.9 * display else 'FAIL'} launch {launch + 1}: {rate:.1f} fps")
            if run.returncode != 0:
                print(run.stderr[-2000:], file=sys.stderr)
                return 1
    return 0 if all(rate >= 0.9 * display for rate in rates) else 1


if __name__ == '__main__':
    sys.exit(main())
