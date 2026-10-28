# Keyboard activation chords

Enter activates a button as it goes down, as a browser's button does;
Space activates as it comes up. A pending Space activation survives unrelated
key releases, and a matching release consumes it before calling the
activation handler; an activation callback may safely begin another press.
Prevented matching releases and focus loss still cancel it. Enter pressed
while Space is held activates and drops the pending Space; repeats do
nothing.

Active styles follow that same key ownership. Pointer and keyboard active state
are tracked separately, so releasing one input does not erase the other held
input's visual state. Pointer leave does not cancel keyboard pressed styling;
blur clears both.

Core tests cover unrelated keys, repeats, overlap, prevented release, blur and
reentrant activation. Composed tests cover active colors and mixed pointer/key
state. A native X11 runner sends real Space/Enter chords, checks activation
counts and held/released colors, cancels a pending press through focus changes,
and verifies an event handler can prevent activation.

```sh
cargo build -p zgui-desktop --example keyboard_chords --locked
python3 scripts/keyboard_chords_smoke.py target/debug/examples/keyboard_chords \
  --output /tmp/zgui-keyboard-chords
```

The runner owns its Xvfb/Openbox processes and isolates desktop settings.
Archived source inputs, runner, hashes, commands, native logs/screenshots and
integrated checks accompany this record. Native rendering uses Mesa llvmpipe;
macOS cross-checking does not establish native macOS keyboard behavior.

The final build passes **469 tests** (two opt-in tests ignored),
**29 doctests**, strict workspace Clippy, formatting and the macOS ARM64
cross-check. Native keyboard chord checks pass. All 141 source hashes match
both the archive and workspace after execution.
