# Translation normalization

`Scene::set_transform` now normalizes each nonfinite coordinate to zero before
comparison, damage tracking and geometry revision changes. Finite coordinates
retain their value independently; signed zero is canonicalized. Fluent
`translate` styles use the same scene boundary.

The direct regression covers NaN/infinite inputs, finite-axis preservation,
restored hit testing and damage, and idle repeated writes. Component tests cover
reactive sparse overrides, actual reevaluation without invalidation, and restoring
the base translation when an override is removed. GPU tests compare incremental
and full-redraw pixels and verify that translation-only changes reuse isolated
layer content, with no work on repeated normalized values.

This normalizes nonfinite inputs, not every possible overflow from accumulating
large finite coordinates. It adds no clamping to otherwise finite translations.

Build inputs and smoke scripts are archived with a source manifest; metadata
records exact commands and binary/source hashes. Native image-fit and hidden
window smoke runs exercise the integrated build on owned Xvfb/Openbox with Mesa
llvmpipe. The direct/component/GPU regressions establish invalid-input behavior;
those native smoke tests alone do not. Native macOS execution remains unverified.

The recorded build passes **460 tests** (two opt-in tests ignored),
**29 doctests**, strict workspace Clippy, formatting and the macOS ARM64
cross-check. Both native smoke suites pass. All 138 source hashes match
the archived inputs and workspace after execution.
