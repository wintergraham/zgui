# Portable native component-host smoke

The two-second idle and streaming-plus-scrolling runs both close cleanly on
Linux X11. Idle reports zero model ticks and 17 mounted rows; combined reports
119 ticks and 19 rows. This exercises native host startup, model scheduling,
bounded virtualization and close callbacks. It does not measure presented
frames or exercise user input. GPU pixel tests remain separate.

The exact harness, logs, result JSON and executable hash are retained here. The
executable and source proof come from the linked measured-framework comparison.
Two initial harness invocations failed before this successful run: one specified
a nonexistent binary filename; the other inherited a stale Wayland display.
The recorded successful command uses the real binary and clears WAYLAND_DISPLAY.
Neither failure represents a completed native workload trial.

The configured macOS CI runs this harness and mandatory Metal readback tests,
retaining logs on failure. Native macOS execution has **not** occurred here;
Linux success does not validate Metal, AppKit, IME or accessibility on macOS.
