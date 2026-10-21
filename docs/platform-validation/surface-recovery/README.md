# Surface recovery and native host lifecycle

Repeated Lost/Outdated acquisition no longer turns a second recoverable surface
change into an application error. Each call still tries acquisition at most
twice and performs at most one recovery. If another surface change races that
recovery, it returns the existing retryable Timeout status. The desktop host
retains the rendered target and uses its bounded 16/32/64 ms retry schedule,
then sleeps until fresh damage or exposure. Recreation and validation errors
still propagate. This follows the recoverable status distinction in
[wgpu 29.0.4](https://github.com/gfx-rs/wgpu/blob/v29.0.4/wgpu/src/api/surface_texture.rs).

Fault-injection checks exercise all Lost/Outdated pairs, later success,
persistent changes, actual timeout/occlusion and fatal errors. A separate
ignored native X11 test creates real windows and GPU renderers, verifies repeated
suspension releases native resources and clears IME/capture/retry state, retains
editor identity and model updates, restores focus and renders after recreation.
It passes in an isolated Xvfb process; CI runs it separately and uploads its log.
This directly invokes host lifecycle handling and does not simulate machine
sleep or prove recovery from a hardware-driver fault.

The complete workspace suite, doctests, strict Clippy, formatting and macOS ARM64
all-target cross-check pass. `counts.json` records exact totals; commands and
logs are retained. `sources.tar.gz` and `sources.json` freeze 172 source inputs,
verified unchanged across these checks. The native test log is retained as
`native-host-suspend.log`. Native macOS execution remains unverified.

After the full run, the desktop crate landing-page example was switched to
components and child views. The final documentation source and hash are retained
separately; all four desktop doctests and formatting pass after that change.
That documentation edit changed no production behavior.
