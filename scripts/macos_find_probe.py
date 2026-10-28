#!/usr/bin/env python3
"""Find in page on macOS, by AppKit key events sent to the app's own window.

    python3 scripts/macos_find_probe.py [--target-dir DIR]

Builds the selectable_text example and runs it with --find-probe: it sends
itself ⌘F, types "line 7777)" into the bar, presses Return and Escape, and
prints find's status as it changes. Checks that ⌘F opens the bar, that the
one match (a transcript row of 10,000 the list had not mounted) is found and
counted, that the transcript scrolls to it, and that Escape closes it.

The keys go to the app's own key window (`-[NSApplication sendEvent:]`), not
the system: the user's keyboard and other apps hear nothing. It is launched
as a direct child, from /tmp, with a clean environment. If a system dialog
appears, stop and report it; never click it. macOS only.
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
    subprocess.run(['cargo', 'build', '-q', '-p', 'zgx-desktop', '--example', 'selectable_text', '--locked'],
                   cwd=ROOT, env=env, check=True)
    with tempfile.TemporaryDirectory(prefix='zgx-find-') as stage:
        binary = pathlib.Path(stage) / 'selectable_text'
        shutil.copy2(pathlib.Path(args.target_dir) / 'debug/examples/selectable_text', binary)
        clean = {key: os.environ[key] for key in ('HOME', 'TMPDIR') if key in os.environ}
        clean['PATH'] = '/usr/bin:/bin'
        out = subprocess.run([str(binary), '--find-probe'], cwd='/tmp', env=clean, capture_output=True,
                             text=True, timeout=60).stdout
    lines = re.findall(r'^FIND (.*)$', out, re.M)
    fields = [dict((k, v.strip('"')) for k, v in re.findall(r'(\w+)=("[^"]*"|\S+)', line)) for line in lines]
    checks = {
        'opened': any(f['open'] == 'true' for f in fields),
        'found and counted': any(f['query'] == 'line 7777)' and f['count'] == '1' and f['done'] == 'true'
                                 for f in fields),
        'scrolled to it': any(f['query'] == 'line 7777)' and abs(float(f['offset']) - 7776 * 22) < 300
                              for f in fields),
        'the current match marked': any(f.get('current_marked') == '1' for f in fields),
        'closed, its marks gone': bool(fields) and fields[-1]['open'] == 'false' and fields[-1]['marked'] == '0',
        'done': 'FIND_PROBE_DONE' in out,
    }
    print('\n'.join(lines))
    for name, ok in checks.items():
        print(('ok  ' if ok else 'FAIL') + ' ' + name)
    return 0 if all(checks.values()) else 1


if __name__ == '__main__':
    sys.exit(main())
