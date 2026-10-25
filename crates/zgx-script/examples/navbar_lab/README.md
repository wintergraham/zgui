Ask Navbar, native zgx
=======================

Run from the zgx checkout:

```sh
rtk cargo run --release -p zgx-script --example navbar_lab
```

Source is Ask's motion library page `experiments/react/src/pages/NavbarPage.tsx`, driven by
`experiments/react/src/controls/navbar.js` and styled by `design/navbar.css`, at
`codex/react-native-mvp` `236911a`. Every Ask checkout on this machine carries the same page and script
(`NavbarPage.tsx` blob `125685ec`, `navbar.js` blob `c41cdbd`). `source.json` records checksums of the inputs
and the logo tints. Refresh favicons, icons, tints and checksums with:

```sh
rtk proxy python3 scripts/import_navbar.py /path/to/ask
```

The port keeps `navbar.js`'s structure and names: the dummy browser (tabs, history, timed loads, bookmarks,
per-origin permissions, cookies, a camera request), the omnibox model (suggestions, inline completion, Ask,
`@tabs` and site modes), and the floating bar with every effect it drives:

- the full bar and the identity pill, collapsing on scroll with the source's hysteresis, expanding on click;
- dragging the bar or pill to the left, centre or right dock, settling on the same damped spring;
- the address identity and its hover morph into the raw URL (gaps that grow, separators that close, the host
  relaxing from 500 to 400 weight), and the cross-fade between URLs;
- the open transition (scrim with backdrop blur, the omnibox scaling in, rows staggering), the Search ↔ Ask
  morph of one surface, and the Ask composer with the page attached as a drifting dithered chip;
- the site menu with its sliding highlight and value submenus, the extensions menu with pinning, the bookmark
  editor, the permission prompt and cookies;
- load progress on the connecting curve, the finish glide capped at 1.5 widths per second and the six-frame
  settle, with the dither wash tinting toward the logo's colour near its leading edge;
- the agent-acting aura and hand-back bar, the toast, reader mode, offline, and the lab controls (Replay,
  Scrub, Slow loads, Camera request, Agent acting, Offline, Scroll to top) with the state readout.

`dither.luau` is a full port of `design/dither.js` (Bayer thresholds, value noise, fbm, the lit orb, noise
blotches and the edge aura) run per pixel on the GPU, so the skeleton favicon, wash, chips and aura animate
on the compositor. Logo tints are `navbar.js logoTint`, precomputed with the box filter that matches
Chrome's `drawImage` within two levels per channel. Motion values are states animated by the anime engine,
scaled by Slow and zeroed by Reduce motion; nothing runs at rest. ⌘L is a View menu command, so it works
wherever focus is.

Focused verification:

```sh
rtk cargo test --release -p zgx-script --test navbar_lab
NAVBAR_LAB_SNAPSHOTS=/tmp/navbar rtk cargo test --release -p zgx-script --test navbar_snapshots -- --nocapture
```

The snapshot test renders 19 states through the GPU renderer with the window's font shaper and writes the
window frame as PPM, for comparison with the same states captured from the Ask motion lab in Chrome (rest,
hover, open, typed with completion, Ask, @tabs, site menu and submenu, extensions, bookmark, pill, loading,
new tab, HTTP, internal page, agent acting, permission, offline and cookies). Layout matches the source's
measured geometry to the point; its state readout matches in every state.

Known differences:

- The source's New tab and Offline pages render under the bar in Chrome: the page-in animation's transform
  makes the 0-height document wrapper their containing block. Here they are centred in the page, as their
  `inset: 0` intends (and as they show on first load).
- `backdrop-filter` blurs are drawn (headless Chrome captures skip them, so the pill reads sharper there).
- CSS `line-height: normal` and border widths take layout space in the source; zgx's normal is 1.4em and its
  borders paint without taking space, so the port sets Chrome's normal line height for Geist and pads by the
  border width where the source's content sits inside one.
- CSS transitions on hover colours follow the lab's Slow factor at the time they start.
