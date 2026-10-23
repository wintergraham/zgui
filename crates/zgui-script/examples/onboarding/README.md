Ask · Onboarding, native zgui
=============================

Run from the zgui checkout:

```sh
cargo run --release -p zgui-script --example onboarding
ONBOARDING_STEP=appearance cargo run --release -p zgui-script --example onboarding
```

Source is the "Ask · Onboarding" board (`A0B6S`) of
`ask-designs/designs/ask.pen`: the nine screens from 00 Splash to 07 Ready,
the option masters and the Keychain coach in both of its states.
`source.json` records checksums of the pen file and the board's shaders.
Refresh the browser logos, the Keychain prompt and the icons with:

```sh
python3 scripts/import_onboarding.py /path/to/ask-designs /path/to/html-exports
```

where the second folder holds Pencil "html-css" exports of the board's
screens (Pencil outlines its Feather glyphs into filled paths there).

What works
----------

- **Splash**: `splash-space.glsl` ported to WGSL (planet, stars, ringed
  planet, moon, a shooting star every nine seconds, at the board's 12 fps).
  The "ask." wordmark is solid text under a second pass of the scene drawn
  only where the wordmark's own dither is off. Click, Enter or three seconds
  move on.
- **Title bar**: × closes the window; the five-step strip lights the current
  step and jumps back to any step reached; Skip setup goes to Ready.
- **Choose browsers**: the eleven browsers and "From a file" toggle; Skip
  clears them and Continue goes straight to Appearance.
- **Choose data**: the source badges and every row's counts follow the
  picks; parts nothing picked has are greyed. Import starts the import.
- **Importing**: each source in turn, part by part, with progress, time left
  and a summary when done. A Chromium browser's saved passwords stop for the
  **Keychain coach**: type (only the count of characters is kept) and press
  Enter, then Always Allow or Allow; Deny skips that browser's passwords;
  "Ask again" starts over; "?" explains the prompt. Dismissing it leaves the
  import waiting, with "Allow access" on the row and the footer to reopen it.
  The import keeps running on later steps, in the footer's quiet status.
- **Appearance**: Dark, Light and System restyle the whole window through
  two registered themes; Tint recolours every dither; Reduce motion stops
  the splash's drift, spinners and slides; Intensity and Grain change the
  textures' density and pixel size everywhere. The preview's button books.
- **Agent**: the four levels with their meters; the breakdown is derived
  from the level's rules. Customize per action opens 06B; Turn AI off and
  on. Left and Right change the level.
- **Choose per action**: the presets set every rule; each rule's menu
  (Always ask, Once per site, Don't ask) changes one, and the level becomes
  whichever preset the rules now match, or custom.
- **Ready**: the live import status, the agent summary,
  Make Ask default, and Open Ask, which fades the window and closes it.

Enter takes each step's primary action and Escape goes back.

Differences from the board
--------------------------

- Only the splash has a shader background; the other steps sit on the plain
  window colour (the board's dither and import fields behind them are left
  out). The Appearance preview, its focus halo and the orbs keep their
  dithers.
- The board's squircle corners are drawn as round corners of similar size.
- The Welcome strip shows "Importing 68%" on every step; here the quiet
  status appears once an import is running.
- The progress strip on the board is flat; here it marks the current step.
- 06 Agent's breakdown lists up to five items per column (Assist asks before
  publishing too, as 06B says), so its rows come from the rules.
- The Keychain prompt is the board's picture of macOS's dialog, with its
  field and buttons made live over it.

Focused verification:

```sh
cargo test --release -p zgui-script --test onboarding
```
