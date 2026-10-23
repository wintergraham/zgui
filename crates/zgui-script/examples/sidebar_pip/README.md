zgui · Sidebar & PiP, native zgui
=================================

Run from the zgui checkout:

```sh
cargo run --release -p zgui-script --example sidebar_pip
```

Imported from `ask-designs/designs/ask.pen`, board "Sidebar · Navigation &
PiP" (`Ap3Cr`): the expanded master (`zuxPa`), the current Agent PiP's three
states (`QasjD`: master, stacked, stack expanded), the pinned trays spec
(`d64X8`), the space switcher spec (`ydknO`) and the favicon resolve
(`Ozgfl`). The PiP variations (`cFBsB`) are left out. The collapsed rail
follows the `Left sidebar · Collapsed` component (`vWj9y`) in the expanded
sidebar's order, as the board's note asks. Favicons are the board's
`assets/favicons`; icons are Feather, the board's library.

The board's GLSL fills are ported to WGSL in `dither.luau`: `bayer-dither`
(orbs, the space mark, the agent's focus glow, live thumbnails),
`dither-fog-sweep` (an open pin), `tab-loading-bg` (a loading row) and the
blob half of `favicon-resolve`. A fill cannot sample an image, so the favicon
focuses as an image over the blob instead: blur to sharp, small to full
size, grey to colour, on the shader's timeline.

Everything is interactive, with canned data:

- Spaces: the bottom switcher (hover for the tooltip, click to switch),
  the topbar menu, or ⌃1 ⌃2 ⌃3. Each space keeps its own pins, chats, tabs
  and selection.
- Pins: hover, click to open (the brand's dithered fog is the active state),
  Tab to the focused ring, right-click an open pin to close it.
- Chats: select one; the pencil starts a new chat.
- Tabs: select; hover for close; mute the playing tab; a sleeping tab wakes
  and loads; right-click for Reload / Close. Groups fold and close. A
  loading tab's favicon resolves in the row, the rail and the page together.
- Search (⌘K, ⌘T, or New tab): an address opens it, anything else searches
  Google, as a new tab that loads.
- The sidebar toggle (⌘S) collapses to the rail and back.
- PiP: agents take a step every few seconds. +N expands the stack (⌘E),
  the chevron folds it; a row or Review brings that agent forward; Review
  on the front card approves the waiting agent; Stop removes one, Close
  clears a finished one; one agent left is the master card, and its expand
  button opens the agent's tab.
- ⌘W closes the tab, ⌘R reloads it, ⌥↑/↓ steps through tabs, ⇧⌘R resets.

Headless checks, with GPU snapshots when `SIDEBAR_PIP_SNAPSHOT` names a file
prefix:

```sh
cargo test --release -p zgui-script --test sidebar_pip
```
