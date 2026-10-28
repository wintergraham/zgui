# zgx.zeron.sh: design brief

**Direction: restraint.** zgx does exactly the work a change needs and nothing more, and the site should read the same way. That means one typeface, a few sizes, hairline structure and plain sentences, with colour only where something is live. Polish comes from proportion and consistency, not ornament.

## What we take from onkeiki.com

We take its discipline, not its content.

- **One sans, weight and tone instead of typefaces.** Headings are a single paragraph: a dark lead sentence, then a lighter continuation in the same size ("**One agent, every channel.** Customers text it on…"). Sizes stay modest: 36 px for the hero lead and 26 px for section leads, with tracking pulled in about 3.5%. Body text is 15–16 px.
- **A ruled page.** Content sits in a centred column bounded by hairline vertical rules, and full-width hairlines separate sections. It reads like a drawing, not a stack of cards.
- **Two columns per section.** Text sits on the left: a small icon-tile label ("Conversations"), the lead heading, and a list in hairline rows. On the right is one large, softly tinted panel that shows the product itself.
- **Numbers as type.** A big figure with a tiny label above it, with no box around it.
- **Small, quiet controls.** Buttons are 32–36 px tall, with one filled primary per view and outlined secondaries. Nav links sit centred with a small icon; the call to action is on the right.
- **A footer of link columns.** Nothing else competes with it.

## Type

- **Geist** for everything, with **Geist Mono** for code, keys and the few figures that are data.
- **Scale:** 13 (labels, captions), 15 (UI), 16 (body), 20 (lede), 26 (section lead), 36–44 (hero lead).
- **Weights:** 600 for leads, 450–500 for continuations and body, 500 for labels.
- **Tracking:** −0.035em on leads, normal for body.

## Colour

- **Light:** white ground, ink `#11231b` (a deep green-black), muted ink `#4d6157` for continuations and body, and hairlines at 10% ink.
- **Dark:** its own palette rather than an inversion. Ground `#0a0f0c`, ink `#e9f1ec`, muted `#97ab9f`.
- **Accent:** one green, `#0b7a4f` in light and `#4ad69c` in dark. It's used for primary actions, zgx's marks in charts, the current page and focus.
- **Panels:** product panels sit on a faint mint-to-sand wash, and code sits on white or near-black inside them.
- **Contrast:** every text pair is AA or better, and code tokens are checked by `scripts/check-contrast.mjs`.

## Space

- **Rhythm:** an 8 px base and 96–120 px between sections.
- **Columns:** the ruled column is 76 rem wide with 40 px gutters to its rules. Columns split 5/7 (text and panel) on desktop and stack on phones.

## Words

Talk only about zgx: what it is, why it's fast, what you can build with it, and how to start. Never describe the site or how its docs are made. Leave out numbered eyebrows, counters on cards and cute section titles. Use short, concrete, confident sentences.

## Docs

The same restraint applies inside the docs:

- **Titles:** Geist semibold, set as plainly as the home page's leads.
- **Sidebar:** sits on a faint tint.
- **Current page:** marked with the accent only.
- **Tables and callouts:** ruled, not boxed.
- **Ornament:** nothing that doesn't help someone read.

## Motion

- **Sections:** fade in 8 px as they enter.
- **Chart:** marks move from zgx's line to their values.
- **Hover:** transitions take 150 ms.
- **Reduced motion:** with `prefers-reduced-motion`, everything stays still.
