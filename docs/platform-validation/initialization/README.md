# Component initialization ownership

An initial binding could panic after `Ui::mount` returned when an outer batch or
running effect deferred its evaluation. The synchronous construction guard had
already committed, leaving the failed public mount and its resources alive.

Initial bindings now carry a temporary ownership scope for the exact public
mount. Their first callbacks restore that scope while running, so bindings made
by initial conditional/keyed children share the cleanup boundary. A first-run
panic removes that mount, its subscriptions and retained resources. Cleanup
restores the previous scope before invoking resource destructors. Successful
initial callbacks release their scope reference; later updates run normally.

`Ui::bind` registers ownership before its initial callback runs. A callback that
unmounts its owner therefore cannot resurrect an owned subscription afterward.
Binding an already removed node is rejected before creating an effect. Reactive
signal tracking now checks that the running effect still exists before adding a
subscription; a callback that disposes itself cannot leave orphan subscriptions
through subsequent reads.

Tests cover outer batches, mounting from an active effect, retained resource
counts, exact sibling preservation, initial conditional/keyed bindings,
self-removal, stale owners, released initialization scopes and retries.
Top-level construction failures retain the prior document. If `render` already
returned and replaced the old document before a deferred binding fails, cleanup
removes the failed new mount but does not restore the old document. Later update
panics keep the existing view mounted and can be retried after correcting the
model. Arbitrary application state writes are not rolled back. User panics
propagate; the native host does not convert them to `Application::run` errors.

The native read-only editor, model normalization and million-row virtual-list
keyboard suites exercise ordinary successful component mounting and updates.
They do not inject a user panic into the native event loop. Native Linux checks
use owned Xvfb/Openbox sessions and Mesa llvmpipe; native macOS and hardware GPU
validation remain outstanding.

Source archives, hashes, exact commands, native snapshots and validation logs
are recorded alongside this document.

The final build passes **509 workspace tests** (two opt-in tests ignored),
**29 doctests**, strict Clippy, formatting and the macOS ARM64 cross-check.
All three native suites pass. All 155 source hashes match archive and
workspace after execution.
