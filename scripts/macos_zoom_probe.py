#!/usr/bin/env python3
"""A window's zoom on macOS, by AppKit key events sent to the app's own window.

    python3 scripts/macos_zoom_probe.py [--target-dir DIR]

Builds the zoom_probe example and runs it with --drive: it sends its own key
window ⌘=, ⌘=, ⌘- and ⌘0, printing its zoom, its viewport in points and a
text's size as they change. Checks that the keys step through Chrome's
levels (110%, 125%, 110%, 100%), that the viewport follows the zoom, and
that text keeps its size in points.

The keys go to the app's own key window (`-[NSApplication sendEvent:]`), not
the system. It is launched as a direct child, from /tmp, with a clean
environment. If a system dialog appears, stop and report it; never click
it. macOS only.
"""
import argparse
import os
import pathlib
import platform
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--target-dir', default=os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target')))
    args = parser.parse_args()
    if platform.system() != 'Darwin':
        print('macOS only')
        return 2
    env = dict(os.environ, CARGO_TARGET_DIR=args.target_dir)
    subprocess.run(['cargo', 'build', '-q', '-p', 'zgx-desktop', '--example', 'zoom_probe', '--locked'],
                   cwd=ROOT, env=env, check=True)
    with tempfile.TemporaryDirectory(prefix='zgx-zoom-') as stage:
        binary = pathlib.Path(stage) / 'zoom_probe'
        shutil.copy2(pathlib.Path(args.target_dir) / 'debug/examples/zoom_probe', binary)
        clean = {key: os.environ[key] for key in ('HOME', 'TMPDIR') if key in os.environ}
        clean['PATH'] = '/usr/bin:/bin'
        out = subprocess.run([str(binary), '--drive'], cwd='/tmp', env=clean, capture_output=True,
                             text=True, timeout=60).stdout
    lines = [dict(re.findall(r'(\w+)=(\S+)', line)) for line in re.findall(r'^ZOOM (.*)$', out, re.M)]
    zooms = [line['zoom'] for line in lines]
    sizes = {line['zoom']: tuple(map(float, line['viewport'].split('x'))) for line in lines}
    base = sizes.get('1.00', (0., 0.))
    checks = {
        'steps through Chrome\'s levels': zooms == ['1.00', '1.10', '1.25', '1.10', '1.00'],
        'the viewport follows the zoom': all(
            abs(sizes[z][0] - base[0] / float(z)) <= 1.5 and abs(sizes[z][1] - base[1] / float(z)) <= 1.5
            for z in ('1.10', '1.25') if z in sizes),
        'text keeps its points': len({line['text_points'] for line in lines}) == 1,
        'done': 'ZOOM_DONE' in out,
    }
    print(out.strip())
    for name, ok in checks.items():
        print(('ok  ' if ok else 'FAIL') + ' ' + name)
    return 0 if all(checks.values()) else 1


if __name__ == '__main__':
    sys.exit(main())
