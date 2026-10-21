# Editor layout reuse

Editor interaction reuses one shaped layout for the focused editor in each `Ui`.
Selection, caret affinity and scroll position query that layout without shaping
unchanged text again. The cache compares the displayed text, font attributes,
font size, wrapping width and installed shaping-engine revision. Text edits,
composition changes, typography and width changes cannot reuse stale geometry.

The entry is owned by the UI, rather than retained by external editor handles.
Blur, removal and explicit text-geometry refresh release it. Admission limits
text to 64 KiB and accounted entry weight to 1 MiB; unknown-weight custom layouts
are not retained. These limits describe cache accounting, not total process RSS
or the transient memory required to shape a document. Larger documents remain
supported through uncached shaping.

This optimization removes repeated shaping. It does not eliminate the existing
display-string construction or guarantee constant-time text comparisons. The
streaming/list comparison remains a separately archived workload; its results
are not evidence of editor-interaction performance.

The counting-shaper probe recorded the following calls after initial allocation
and focus settled. Its [before source](before.rs) and [recorded output](before.log)
preserve the original scenario; the current regression tests assert reuse and
check actual scrolling and selection changes.

| Interaction | Before | With reuse |
| --- | ---: | ---: |
| Five unchanged refreshes | 5 | 0 |
| Three selection changes | 3 | 0 |
| Three overflow wheel steps | 3 | 0 |
| End, Down, Home, Up | 8 | 0 |

These are shaping-call counts, not CPU-time or allocation measurements. A native
cosmic-text/GPU regression also checks zero additional editor-shaper calls during
navigation and compares damaged rendering with full repaint pixels. Replacement
shaping engines now refresh caret metrics even if the editor's bounds stay equal.

[Native smoke and consolidated validation](../../platform-validation/editor-shaping/README.md) preserve source/binary provenance and checks.
