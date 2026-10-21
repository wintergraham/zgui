# Retained editor text during selection refresh

Editor refresh previously cloned committed text to prepare display text, built
a fresh paint-text allocation even when identical, and cloned the semantic node
before assigning another full semantic value. Selection-only changes therefore
copied the document repeatedly.

Refresh now borrows committed text unless composing preedit, and reuses the
existing paint `Arc<str>` when its content is unchanged. The new
`Semantics::update_text_input` compares editor value, selection and read-only
state directly, updates only those fields, and increments the semantic revision
once when something changes. It retains unchanged strings and reuses capacity
for changed values. Missing semantic nodes remain a no-op.

A thread-local counting allocator regression measures only the calls under test.
A 256 KiB semantic value needs no allocation for repeated equal, selection,
read-only or same-sized text updates; growth beyond capacity allocates once,
and later shrink/regrowth reuses capacity. An integrated focused-editor test
uses a 32 KiB Unicode document with 8,192 fallback cells, fitting the existing
bounded shaping-cache policy. Repeated selection/read-only refreshes allocate
nothing as large as the document and preserve paint Arc identity and semantic
string storage. This is not a claim that all editor refreshes allocate nothing:
geometry may allocate, and cold or over-budget layouts may reshape.

AccessKit consumer coverage applies selection and read-only incremental updates,
checking complete Unicode text, selection, retained text-run IDs and actions.
The live AT-SPI probe rereads editor text after caret and selection changes.
Native model normalization and IME smoke suites also exercise display-text and
preedit correctness on the integrated build.

Native accessibility projection still constructs owned AccessKit node values
and caches cloned nodes. Those separate full-value copies are not removed by
this core optimization. No whole-framework timing or RSS improvement is claimed
from these allocation regressions. Native Linux uses owned Xvfb/Openbox sessions
and Mesa llvmpipe; native macOS and hardware GPU validation remain outstanding.

The integrated build passes **520 workspace tests** (two opt-in tests ignored),
**29 doctests**, strict Clippy, formatting and the macOS ARM64 cross-check.
All three native suites pass, including the live AT-SPI text-preservation checks.
All 158 source hashes match archive and workspace after execution.
