# zgx patch to accesskit_windows 0.34.0

Vendored from crates.io `accesskit_windows` 0.34.0 (MIT OR Apache-2.0; the
published crate ships no license files) and applied through
`[patch.crates-io]` in the workspace `Cargo.toml`. Changes are marked
"zgx patch" in the source.

## Why

Two gaps Narrator meets in zgx's windows. AccessKit gives a heading's level to UI Automation only as the generic
`Level` property and the ARIA `level`, never as `HeadingLevel` (UIA
property 30173). Narrator's heading navigation (scan mode H, the headings
list) and Windows' other screen readers go by `HeadingLevel`, so zgx's
headings read as "heading" but cannot be moved between. Chromium and
WebView2 set it. 0.35.1 and upstream main still leave it out.

## What

1. `heading_level` in `src/node.rs`: a heading's level 1 to 9 as
   `HeadingLevel1` to `HeadingLevel9`, exposed as
   `UIA_HeadingLevelPropertyId`. Other nodes keep UIA's default,
   `HeadingLevel_None`.
2. A test, `src/tests/heading.rs`, reading it through the UIA client.
3. `name` in `src/node.rs`: a table's cell or header (`Cell`, `GridCell`,
   `ColumnHeader`, `RowHeader`) with no label, or an empty one, is named by
   its visible descendants' text, as ARIA names cells and headers and as
   Chromium does. AccessKit derives a name from contents only for buttons,
   check boxes, links, menu items and radio buttons, so a zgx grid's cells,
   whose text is a child, had no UIA Name: Narrator's table navigation read
   them as blank. Tested by `src/tests/cell_name.rs`.
4. A form field's error (`aria-invalid`, `aria-errormessage`), from the
   vendored accesskit_consumer (vendor/accesskit_consumer/ZGX_PATCH.md):
   `IsDataValidForForm` is false while the field is invalid; its error
   message's text follows the description in `FullDescription`, so
   Narrator reads it with the field ("Email, edit, An email is required,
   Invalid"), and `DescribedBy` gives the nodes that describe it
   (`src/node.rs`, `description`, `is_data_valid_for_form`, the
   `DescribedBy` property). Not `ControllerFor`, as Edge gives
   `aria-errormessage`: Narrator reads an edit's `ControllerFor` as a
   suggestion list ("Suggestions available"). The crate's `accesskit_consumer` dependency
   is the vendored one (`path`), so it builds alone too.
5. Events a client listens for, and no others. UIA tells a fragment root
   which events clients add handlers for (`IRawElementProviderAdviseEvents`,
   in `src/node.rs`), and the context counts them (`Context::advise`).
   After a tree update, events no client listens for are dropped
   (`Context::wanted`; text changes before their elements are made).
   - Focus changes are always raised: legacy WinEvent clients follow them.
   - Everything is raised while a screen reader runs (`SPI_GETSCREENREADER`):
     it may take events through the MSAA bridge, where no handler is advised.
   - Why: the touch keyboard's host (TextInputHost) attaches to the focused
     window, listening for focus, structure, selection, menus and a few
     properties, but not text or name changes. A dashboard's live labels
     raised about 1,200 text changes and 830 name changes a second anyway,
     and UIA called back into the provider about ten times for each: about
     18,000 calls and 30 ms of the UI thread a second, for no one. Chromium
     does the same through `AdviseEventAdded`.
   - Tested by `context::tests`. A client that subscribes later receives
     what it subscribed to: a native one counting name changes on that
     dashboard received 2458 in 3 s, against 2489 before the patch.

Run the crate's tests on Windows with
`cargo test --manifest-path vendor/accesskit_windows/Cargo.toml --locked`.
Upstream's `simple::focus` brings its window to the foreground, and fails
when Windows refuses that (a run from a background terminal).

Drop the patch when an upstream release does the same.
