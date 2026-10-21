# Lifecycle cleanup validation

The lifecycle audit reproduced and fixed the following ownership problems:

- Removing a widget could run listener-capture destructors before detaching its
  scene subtree. Reentrant removal then accessed a stale node. Geometry is now
  detached before user destruction; metadata and owned resources are released
  afterward. The [original failing regression](listener-removal-before.log)
  records the stale-node panic.
- Component task pruning invoked executor completion checks and guard destructors
  under a mutable task-list borrow. Reentrant spawning panicked. Pruning now runs
  on a detached vector, preserving its capacity and merging reentrant additions.
- Timer cancellation/replacement and background-task waker replacement could
  execute user waker destruction under scheduler/result locks. Clone and drop
  callbacks now run outside those locks. Regression probes assert both actual
  destruction and successful nonblocking acquisition of the relevant lock.
- Retained `WindowFactory` clones kept the shared clipboard after manager shutdown.
  Shutdown now takes and drops it explicitly, outside the shared borrow.

Four public component regressions also cover repeated keyed/provider/slot
teardown with retained handles, sibling removal, task-guard destructor reentry,
and completion-callback reentry. Live task guards remain owned until unmount;
retained capabilities cannot resurrect a removed component.

The [native clipboard regression](native-clipboard.log) uses a real X11 clipboard
and owned Xvfb display, constructs and drops the window manager, then verifies
that a surviving factory is inert and its shared clipboard is empty. It does not
exercise a complete application-window close sequence or a Wayland compositor.
The test is ignored by display-free workspace runs and runs separately in Linux
CI. No GPU device is required for this native resource test.

Consolidated logs record **426 passing workspace tests**, **28 documentation
examples**, strict Clippy, formatting and macOS ARM64 cross-compilation. Two tests
are ignored in the ordinary workspace suite: independent GPU stress and this
separately executed native clipboard test. [Metadata](metadata.json),
[source manifest](source-manifest.json) and [source archive](source.tar.gz) preserve
128 unchanged build inputs and the executed native test binary hash.

These checks establish the tested disposal behavior, not a process-RSS benchmark,
OS sleep/wake validation, native macOS runtime coverage or real-driver recovery.
