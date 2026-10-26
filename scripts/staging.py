"""Benchmark apps run from copies outside ~/Documents, in /tmp.

macOS protects ~/Documents (privacy, TCC). An unsigned GUI app is its own
responsible process, and every ad-hoc build of it is a new identity: an app
launched from a checkout under ~/Documents (target/dist/...), or with its
working directory there, can stop mid-run to ask for access to the folder.
That dialog sits over the window being measured, and answering it from a
script once wedged the privacy service for most of an hour (2026-10-25).
So a run launches a copy of each binary staged under /tmp, named by its
contents, with /tmp as its working directory, and never touches the dialog.

    from staging import stage, CWD
    subprocess.Popen([str(stage(binary))], cwd=CWD, ...)
"""
import hashlib, os, shutil
from pathlib import Path

STAGE = Path(os.environ.get('ZGX_STAGE_DIR', '/tmp/zgx-staged'))
# The working directory a staged app runs in.
CWD = '/tmp'


def clean_env(**extra):
    """The environment a measured app launches with: the user's home, path and
    temporary directory, and `extra`. Nothing else of the launching shell's
    (an agent's variables, a build's) reaches the app or its measurement."""
    env = {k: os.environ[k] for k in ('HOME', 'PATH', 'TMPDIR', 'USER', 'LANG') if k in os.environ}
    env.setdefault('TMPDIR', '/tmp')
    return {**env, **extra}


def stage(binary):
    """`binary`, copied under STAGE (once per content), as the path to launch."""
    binary = Path(binary)
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()[:16]
    staged = STAGE / digest / binary.name
    if not staged.exists():
        staged.parent.mkdir(parents=True, exist_ok=True)
        partial = staged.with_name(staged.name + '.partial')
        shutil.copy2(binary, partial)
        partial.rename(staged)
    return staged
