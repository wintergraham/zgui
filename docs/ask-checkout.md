# The zgui checkout used by Ask

Ask's Luau shell uses the `ask/browser-mvp` branch of [wintergraham/zgui](https://github.com/wintergraham/zgui/tree/ask/browser-mvp). This branch includes the native-view integration, Ask UI runner and the local fixes copied from the t3 checkout on 2026-10-23.

Fetch it on Linux or macOS without a local t3 checkout:

```sh
git clone --branch ask/browser-mvp https://github.com/wintergraham/zgui.git
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
| `ask/browser-mvp` | Engine currently used by Ask's Luau shell |
| `codex/ask-t3-exact` | Exact source snapshot before this checkout documentation |
| `ask/browser-mvp-up` | Earlier Ask branch with upstream integration |
| `codex/t3-project` | t3 project checkout, demo windows and profiler work |
| `t3/import-navbar-lab` | Navbar lab and its engine changes |
| `t3/import-controls-motion-lab` | Controls motion lab |
| `worktree-ask-onboarding` | Onboarding demo |
| `feat/frame-profiler` | Latest published profiler branch |
| `fix/native-layers-input-pacing` | Latest published layer and pacing fixes |
| `codex/upstream-main` | Upstream zeronsh/zgui main fetched on 2026-10-23 |

The latest upstream snapshot has 287 commits beyond Ask's base. Integrating it produces conflicts in shared renderer and scripting code, so it remains a separate branch. The existing public `main` and its earlier work remain available.
