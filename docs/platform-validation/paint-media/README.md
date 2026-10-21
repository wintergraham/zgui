# Native paint/media acceptance

The interactive X11 fixture passed: a real button click changes the gradient card,
canvas curve and rotated SVG drawing. The image animation advances, remains
unchanged across multiple frame deadlines while paused, and advances after resume.
`interactive/result.json` records binary hashes and changed drawing regions;
PNG captures were visually inspected. `initial-frames` also contains complete
native gallery captures. Software asymmetric-decoration and SVG transformed,
clipped/isolated partial-versus-fresh frame tests passed in the accompanying logs.

The first harness attempt selected a not-yet-mapped X11 window and ImageMagick
failed to capture it. Its application log is retained in `initial-harness-failure`.
The corrected harness waits for a visible mapped window and a nonblank frame.
This was a harness startup failure, not an accepted rendering result.

This is Linux X11 runtime evidence, not macOS runtime proof or performance data.
The evolving framework build is identified by each executable hash; this archive
does not claim that later source edits were part of those executables.
