# GPU test resource isolation

During virtual-list keyboard integration, the workspace GPU pixel-test process
exited with SIGSEGV while running the default parallel harness. The original log
is preserved. An isolated GPU suite and the exact failed workspace binary under
GDB both subsequently passed all 30 tests; no backtrace was obtained. At that stage no cause was classified. A subsequent
[targeted lifecycle stress investigation](../gpu-lifecycle-stress/README.md)
reproduced a loader crash matching an upstream race and passed three matched
runs with the fixed loader. The original crash had no stack, so identical cause
cannot be proven.

The environment exposed 24 CPUs, no `RUST_TEST_THREADS` override, and a Linux
software Vulkan implementation. The test source had 26 independent device/context
creation sites. Default test scheduling could overlap up to 24 fixtures with
independent device, pipeline, font, driver-thread and cache lifetimes. Actual
concurrency at the crash was not measured.

GPU pixel fixtures now hold a shared test-only mutex, which outlives each fixture's
GPU objects. This bounds resource overlap; it does not change application behavior,
renderer configuration, benchmark settings or historical benchmark results. The
shared-device multi-renderer regression remains intact. A separate fixture starts
four independent device workers together and verifies rendered/readback pixels,
retaining explicitly bounded concurrent-startup coverage. It does not substitute
for an unrestricted parallel stress test.

The initial failure and successful diagnostic reruns are retained here. The final
workspace log records validation with resource isolation and the four-worker test.
Native hardware GPU and macOS runtime validation remain outstanding.
