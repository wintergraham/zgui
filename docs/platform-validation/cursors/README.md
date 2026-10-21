# Native cursor styles

Real X11 pointer movement verifies the named Move, NotAllowed (disabled region),
Crosshair and reactive Wait cursors; leaving restores the named Default and
repeated restoration gives identical pixels. Native cursor images come from
XFixes, not from application event logs. The accepted result records the binary
hash and cursor image hashes. Core tests separately check zero layout/paint damage,
style inheritance and complete removal of custom-cursor hit-test overhead.

The initial harness incorrectly required the restored named Default pixels to
match the X server cursor inherited by winit before any explicit named selection.
winit0.30.13 caches Default initially and leaves that server cursor inherited.
These are different platform states. Both failed observation sets are preserved;
the corrected assertion requires named Default and stable repeated restoration.
No framework cursor workaround or global cursor-theme setting is required.
