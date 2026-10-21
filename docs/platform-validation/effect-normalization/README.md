# Effect normalization

Scene effect updates now normalize invalid floating-point values before equality,
blur tracking and damage checks. NaN opacity resolves to the default `1`; infinite
opacity clamps to its existing endpoint behavior (`+∞` → `1`, `−∞` → `0`). Blur
radius and edge-fade length resolve nonfinite and negative values to `0`.

Previously, NaN opacity remained NaN, so repeated writes always looked different.
Positive infinite filter lengths could also reach paint metadata. Normalized
values now remain finite and repeated equivalent updates are idle.

Core tests cover direct Scene updates and blur tracking. A component regression
forces reactive style reevaluation, checks that explicit invalid overrides use
the documented fallback, verifies zero layout/compositor/damage work on repeated
updates, and restores the finite base when the sparse override is removed.
GPU regressions check normal and isolated rendering, fallback pixels, full versus
incremental equality, and zero draw/upload/layer work for repeated invalid writes.
This policy concerns Effects; it is not a blanket validation guarantee for every
floating-point layout, transform or geometry API.

The source archive, manifest and metadata preserve inputs, hashes and commands.
Native image-fit and hidden-window update smoke runs check the integrated build
on owned Xvfb/Openbox with Mesa llvmpipe. The new normalization behavior is
established by the direct/component/GPU tests, not those native smoke tests alone.

The recorded build passes **457 tests** (two opt-in tests ignored),
**29 doctests**, strict workspace Clippy, formatting and the macOS ARM64
cross-check. Both native smoke suites pass. All 137 source inputs match
the archive and current workspace after execution.
