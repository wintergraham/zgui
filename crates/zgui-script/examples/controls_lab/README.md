Ask Controls, native zgui
=========================

Run from the zgui checkout:

```sh
rtk cargo run --release -p zgui-script --example controls_lab
```

Source is the live Ask `codex/native-react-validation` checkout at
`/Users/winter/.codex/worktrees/native-react-validation/ask`, HEAD `534e0a39`,
including its current uncommitted work. This is newer than
`/Users/winter/Documents/GitHub/ask`. `source.json` records checksums of the
inputs. Refresh fonts, vector icons, licenses and checksums with:

```sh
rtk proxy python3 scripts/import_controls.py /path/to/ask
```

The implementation follows the current `ControlsPage.tsx` Components page:
all 23 cards, their order, catalogue IDs, two-column layout and narrow-window
stacking. Demos cover buttons and loading states, editable fields, segmented
controls, switches, mixed checkboxes, sliders, progress, command palette,
single and multiple selects, shortcut recording, tint selection, status orbs,
inline modal, links, badges, keycaps, avatars, tooltips, setting rows, model
menu, nested context menu, alerts, preview card, navigation, dialog and toasts.

This preview uses native scene nodes, bundled Geist fonts and vector paths.
Interactions remain local. Toasts cap at three, cancel displaced timers,
pause on hover and release their nodes after expiry. The working orb runs at
12 fps while visible and stops offscreen or under Reduce motion. The page
retains its root across interactions and requests no animation frames at rest.

Focused verification:

```sh
rtk cargo test --release -p zgui-script --test controls_lab
rtk cargo build --release -p zgui-script --example controls_lab
```

The software layout capture and the visible macOS GPU window were inspected.
Native text entry, clearing a field and the Save loading feedback were checked
in the running window. The headless fallback text measurer differs from the
window's native font shaper, so the software capture is a layout check.

Known differences: dither noise and tint swatch colors approximate the source;
text editing uses the engine's caret rather than the web demo's gliding caret.
Nested popovers use the engine's below-anchor placement. Menu exits use native
portal dismissal. Slow motion covers button and loading playback; retained
selection and navigation bindings keep their authored timing. Inline demo
actions have the same local preview scope as the source page.

At narrow widths the header hides secondary labels so Slow motion and Reduce
motion remain inside the window. The responsive regression check covers both
header controls at the 620px minimum width.
