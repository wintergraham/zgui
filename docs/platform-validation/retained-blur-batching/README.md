# Integrated retained blur and lifecycle validation

All **600 workspace tests** and **31 doctests** pass, along with strict workspace
Clippy, formatting, and the macOS ARM64 all-target cross-check. Three ordinary
suite tests remain explicitly ignored: independent-device GPU stress and two
native tests requiring isolated displays. The Host suspension/recreation test
was run separately here and passed. The native declarative-effects harness also
passed, with its model results, logs and screenshots retained under
`native-effects/`.

A clean blur elsewhere in the scene no longer forces unrelated damage through
separate per-draw render passes. After blur dependency expansion, the renderer
uses its existing batched path when no actual blur output intersects damage.
Distant streaming text uses one render pass and zero blur passes; touching a
sampling halo still runs both separable blur passes. Differential readbacks
match fresh full renders. New `render_passes` and `blur_passes` counters include
recursive isolated-layer rendering and exclude native presentation. Repainting
an isolated layer still redraws that layer's full target and may rerun its blur.
These counters establish avoided work, not measured GPU latency or CPU savings.

The integrated snapshot also includes bounded surface recovery: a second
Lost/Outdated result yields to the existing host retry schedule rather than
terminating the application. Real recreation/validation errors remain errors.
Native host tests cover resource release, IME/capture cleanup, retained editor
state and focus restoration across repeated suspension/recreation. They invoke
host handling directly; machine sleep and hardware driver faults were not
reproduced.

`checks.json` retains every exact command and exit code. `sources.tar.gz` and
`sources.json` freeze 175 inputs; independent verification matches both archived
and working files at audit time. `environment.json` records toolchain, build
environment and tested native binary hashes. `counts.json` and `audit.json`
record totals and verification. Native rendering uses Mesa software Vulkan on
Linux; macOS has compile coverage and configured Metal/native CI, without native
runtime evidence from this machine.

The latest cross-framework CPU/RSS comparison remains the separately frozen
[36-trial series](../../results/measured-framework-workers4/README.md). It predates
this surface-recovery/blur-batching refinement and benchmarks the shared
fixed-height workload without blur; no new CPU/RSS ranking is inferred here.
