# Typed keyboard actions and timed native replay

The retained component API now supports typed actions, scoped keymaps, context
flags/attributes and parsed predicates, function keys, multi-key sequences,
priority fallback, explicit disabled bindings, and timed ambiguity resolution.
The single active key deadline shares the existing demand-driven host timer.

Validation includes 12 focused integration tests and four dedicated parser/registry
unit tests. They cover pending native text, expiry and mismatch replay without
duplicate insertion, exact/longer precedence, unhandled actions, custom editor
prevention, focus/ownership/hide cancellation, disposal and destructor reentry.

The owned X11/Xvfb native fixture passes five stages:

1. F5 dispatch saves `base`.
2. An unmatched `x` prefix updates the model to `basex` on the one-second host
   timer. The harness checks its model-observer output **before sending another
   key**, so a later key cannot masquerade as a working timer.
3. The `x y` chord saves `basex` without inserting either chord character.
4. The F1/F2 chord invokes the same typed action.
5. A mismatched `x z` sequence replays once, producing `basexxz`.

`native/result.json` and logs preserve the final run. `metadata.json` records the
executable hash and a focused source snapshot. This is not a complete workspace
archive; unrelated framework work was concurrent. There is no native macOS or
Wayland keyboard result in this packet, and external file dragging was not
exercised. Per-file hover/drop/cancel routing has synthetic core coverage.

The preserved failed run used xdotool keysym-based function-key injection. Its
routed event trace shows `alt: true`, so plain-F bindings correctly did not match.
The final harness sends physical function keys through XTest on its own X server
and leaves production modifier matching unchanged. It normalizes only those
three keys on that private display. This correction is test-input control, not
a weakened assertion or a framework workaround.

See [the public action guide](../../actions.md) for the configuration and
precedence contract, and run `scripts/actions_smoke.py` with the built `actions`
example to repeat the native test.
