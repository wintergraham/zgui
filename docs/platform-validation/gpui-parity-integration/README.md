# Integrated GPUI capability validation

Linux workspace tests: **712 passed, 0 failed, 3 ignored**. All **31 doctests**
passed. The two native lifecycle tests were subsequently run individually on
owned X11 displays and passed; the independent-device GPU stress remains opt-in.
Formatting and strict all-target Clippy pass on Linux and with the
`aarch64-apple-darwin` target. The Mac check is cross-compilation, not runtime proof.

Freshly rebuilt native fixtures pass painting/gradient/path/SVG updates,
animated-image pause/resume, cursor styles/restoration, mixed rich text and links,
advanced layout resize, contextual actions/timeouts, typed drag previews, grouped
X11 and Wayland file drops/cancellation/rejection, dynamic menus on both desktops,
and transparent/blur effects. Component host idle/active lifecycle passes on an
explicit X11 session. Real Wayland portal open/folder/save/cancel, prompt and URL
checks pass; closing the owner dismisses its picker and suppresses callbacks.

`checks` and `native` retain command logs and results, including unsuccessful
invocations. `native-binaries.json` identifies the rebuilt executables. The
source archive records the integrated source, harnesses and CI configuration.
The full test run preceded a test-only Clippy syntax correction; the corrected
Unicode validation test was rerun and passed. Production behavior was unchanged.

Preserved issues and corrections:

- Clippy rejected a single-range Vec literal in a Unicode unit test. Equivalent
  iterator construction fixed the lint; strict Linux/Mac retries passed.
- Two native-example build attempts hit the workspace's per-user disk quota.
  Completed test/stale example executables were removed, with source/evidence
  retained; the subsequent native build passed.
- The first host lifecycle invocation inherited a stale Wayland display. A
  second added Xvfb but still inherited Wayland. The accepted invocation unsets
  Wayland and explicitly selects X11; both modes then pass.
- Running both opt-in lifecycle tests in one process hit winit's prohibition on
  recreating its event loop. Their documented individual invocations pass.

No failures were discarded or counted as successful cases. These are functional
checks, not performance trials. Native macOS Metal/menu/dialog/input/accessibility
qualification remains pending a Mac runner; prepared checks are in
`scripts/macos_native_validation.py`. No Mac runtime pass is inferred here.
