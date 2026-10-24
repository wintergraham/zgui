# The zgui checkout used by Ask

Ask's Luau shell uses the `ask/zeronsh` branch of [wintergraham/zgui](https://github.com/wintergraham/zgui/tree/ask/zeronsh): [zeronsh/zgui](https://github.com/zeronsh/zgui) `main` as of 2026-10-24 with Ask's pieces on top. Update it by rebasing those commits onto a newer `zeronsh/main`.

What Ask adds to upstream:

- `ui.native` and `ScriptHost::native`: host-provided platform views (Ask's CEF page) placed in Luau layouts, focus mirrored to AppKit's first responder.
- `inert` (HTML's): a subtree absent for input, the pointer passing to what is below; upstream's `pointer_events` keeps its model.
- `keep_focus`: pressing a node leaves keyboard focus where it is.
- `ui.theme.strict(mode, { skip = { … } })`: the design lint leaves chosen kinds of value alone.
- Box shadows drawn as CSS's: the exact blurred rounded box, outside the element only.
- Truncated text shrinks past its longest word (an ellipsis clips, as `overflow: hidden` does in CSS).
- The Ask UI runner (`examples/ask_ui.rs`), the snapshot runner (`tests/ask_ui_snapshots.rs`) and the Ask labs (Controls, Navbar, Quick ask, Sidebar PiP).

Fetch it on Linux or macOS without a local t3 checkout:

```sh
git clone --branch ask/zeronsh https://github.com/wintergraham/zgui.git
cd zgui
```

For Ask itself, initialize its pinned dependency:

```sh
git submodule update --init --recursive zgui
```

Ask's submodule commit is the authoritative dependency version. Use the [README](../README.md) for Linux system libraries and framework build commands. Fetching the source does not require a display server. Desktop examples need a graphical session, and GPU tests need a supported adapter.

## Preserved t3 work

The source and uncommitted changes from each t3 worktree were copied into these branches. Build output and local agent state are excluded.

| Branch | Work |
| --- | --- |
| `ask/zeronsh` | Engine currently used by Ask's Luau shell |
| `ask/browser-mvp` | Engine Ask used until 2026-10-24, on the 2026-10-21 base |
| `codex/ask-t3-exact` | Exact source snapshot before this checkout documentation |
| `ask/browser-mvp-up` | Earlier Ask branch with upstream integration |
| `codex/t3-project` | t3 project checkout, demo windows and profiler work |
| `t3/import-navbar-lab` | Navbar lab and its engine changes |
| `t3/import-controls-motion-lab` | Controls motion lab |
| `worktree-ask-onboarding` | Onboarding demo |
| `feat/frame-profiler` | Latest published profiler branch |
| `fix/native-layers-input-pacing` | Latest published layer and pacing fixes |
| `codex/upstream-main` | Upstream zeronsh/zgui main fetched on 2026-10-23 |

The existing public `main` and its earlier work remain available.
