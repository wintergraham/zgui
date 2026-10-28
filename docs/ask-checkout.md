# The zgx checkout used by Ask

Ask's Luau shell uses the `ask/v0.1.3` branch of [wintergraham/zgui](https://github.com/wintergraham/zgui/tree/ask/v0.1.3): Ask's pieces (`ask/menu-bar`, on zgx `main` between 0.1.1 and 0.1.2) with the [zeronsh/zgx](https://github.com/zeronsh/zgx) `v0.1.3` release merged in. Update it by merging a newer zgx release tag, preferring upstream where both changed the same thing; `on_aux_click` was Ask's and is upstream's since 0.1.3, so only upstream's remains. The fork keeps its old repository name; `ask/zgx` and `ask/menu-bar` are the earlier bases, and `ask/zeronsh` the same work before the zgui rename (zgui `7d559add`).

What Ask adds to upstream:

- `ui.native` and `ScriptHost::native`: host-provided platform views (Ask's CEF page) placed in Luau layouts, focus mirrored to AppKit's first responder.
- `inert` (HTML's): a subtree absent for input, the pointer passing to what is below; upstream's `pointer_events` keeps its model.
- `keep_focus`: pressing a node leaves keyboard focus where it is.
- `ui.theme.strict(mode, { skip = { … } })`: the design lint leaves chosen kinds of value alone.
- Menu bar: an app's items in macOS's application menu (`AppMenu::APPLICATION`), its own Window and Help menus around AppKit's standard items, and `StandardItem::ToggleFullScreen`.
- Frames drawn every refresh or unlimited (`FrameMode`), counted as presented; a press through a child keeps the pointer; a script's focus or selection set in a handler survives the event's default action; work deferred outside an event waits for its batch.
- A presence's box lets the pointer through while what it shows is hidden; opened documents arrive as file URLs on macOS; native views and cached layers stay on the device pixel grid.
- An input event whose target was removed after it was chosen is dropped rather than panicking ("stale scene node"), and each dispatch forgets the capture, press and focus that name removed nodes.
- Box shadows drawn as CSS's: the exact blurred rounded box, outside the element only.
- Truncated text shrinks past its longest word (an ellipsis clips, as `overflow: hidden` does in CSS).
- The Ask UI runner (`examples/ask_ui.rs`), the snapshot runner (`tests/ask_ui_snapshots.rs`) and the Ask labs (Controls, Navbar, Quick ask, Sidebar PiP).

Fetch it on Linux or macOS without a local t3 checkout:

```sh
git clone --branch ask/v0.1.3 https://github.com/wintergraham/zgui.git zgx
cd zgx
```

For Ask itself, initialize its pinned dependency:

```sh
git submodule update --init --recursive zgx
```

Ask's submodule commit is the authoritative dependency version. Use the [README](../README.md) for Linux system libraries and framework build commands. Fetching the source does not require a display server. Desktop examples need a graphical session, and GPU tests need a supported adapter.

## Preserved t3 work

The source and uncommitted changes from each t3 worktree were copied into these branches. Build output and local agent state are excluded.

| Branch | Work |
| --- | --- |
| `ask/v0.1.3` | Engine currently used by Ask's Luau shell: Ask's pieces with zgx 0.1.3 |
| `ask/menu-bar` | Engine Ask used until 2026-10-27, on zgx `main` between 0.1.1 and 0.1.2 |
| `ask/zgx` | Engine Ask used until 2026-10-26, on zgx `main` after the rename |
| `ask/zeronsh` | Engine Ask used until 2026-10-25, on zgui before the zgx rename |
| `ask/browser-mvp` | Engine Ask used until 2026-10-24, on the 2026-10-21 base |
| `codex/ask-t3-exact` | Exact source snapshot before this checkout documentation |
| `ask/browser-mvp-up` | Earlier Ask branch with upstream integration |
| `codex/t3-project` | t3 project checkout, demo windows and profiler work |
| `t3/import-navbar-lab` | Navbar lab and its engine changes |
| `t3/import-controls-motion-lab` | Controls motion lab |
| `worktree-ask-onboarding` | Onboarding demo |
| `feat/frame-profiler` | Latest published profiler branch |
| `fix/native-layers-input-pacing` | Latest published layer and pacing fixes |
| `codex/upstream-main` | Upstream zeronsh/zgx main fetched on 2026-10-23 |

The existing public `main` and its earlier work remain available.
