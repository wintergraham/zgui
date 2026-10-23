Ask Settings, native zgui
=========================

Ask's Settings page written in Luau and drawn by zgui's native nodes. No
React, JavaScript engine or web view is involved.

```sh
cargo run --release -p zgui-script --example settings
```

The design is Ask's `codex/native-react-validation` Settings lab
(`experiments/react/src/controls/Settings*.tsx`, `Providers.tsx`,
`src/design/settings.css`). `source.json` records the checksums of the files
it was ported from.

All thirteen sections are here: General (account dialog, search engines with
the add-engine dialog), Appearance (theme, tint, reduce motion, the dither card
and its live preview), Tabs & navigation (the scaled window preview), Assistant,
Providers, Models (provider usage, loadout list and editor, model browser,
switching and task models), Confirmations (the pinned stage that plays each
kind of action), Passwords, Extensions, Downloads, Spaces & profiles, About
(update check, versions, licence families, all 726 packages) and Developer.
Settings search covers every setting and jumps to the one you open.

Files:

| | |
| --- | --- |
| `app.luau` | page shell: nav and its pill, search, background dither and entrance, sections |
| `kit.luau` | palette (theme and OKLCH tint), dither fills, orb, toggle, select, rows, segmented control, keycaps, popup layer |
| `model.luau` | every setting as keyed state, restored from and saved to `ui.shared("settings")` |
| `dither.luau` | the Bayer dither (`dither.js`) and the orb as WGSL fills |
| `search.luau` | the settings index, ranking and results |
| `general.luau`, `appearance.luau`, `tabs.luau`, `assistant.luau`, `models.luau`, `confirmations.luau`, `extensions.luau`, `about.luau` | the sections |
| `icons.luau`, `logos.luau`, `licenses.luau`, `assets/` | Lucide icons, provider marks, the package list, fonts and images |

The runner saves preferences as JSON in `target/settings/preferences.json`, or
in the directory `ZGUI_SETTINGS_PROFILE` names.

How the design maps to zgui:

- Theme and tint changes rebuild the page under a 260 ms crossfade, where the
  React version eases every colour. Because the page is rebuilt, colours are
  plain values everywhere, hover styles included.
- Menus and dialogs draw in their own layer over the page. Menus anchor to their
  trigger's right edge, as `.st-menu` does, follow the page as it scrolls and
  close on Escape or a click outside.
- The nav and the Confirmations stage stay pinned while the page scrolls, as
  `position: sticky` does.
- Dither surfaces run their 12 fps clock only while dither animation is on and
  motion isn't reduced. The page requests no frames at rest.

Known differences:

- "System" theme follows dark, because zgui has no system appearance signal yet.
- The agent cursor has no drop shadow.
- Copy in About doesn't write to the clipboard.
- Extension, download and link buttons are previews, as in the source lab.

Tests: `cargo test --release -p zgui-script --test settings`. Set
`SETTINGS_SNAPSHOTS=dir` to write a software-rendered PPM of each section.
That renderer's fallback text measurer differs from the window's shaper, so
the snapshots only check layout.
