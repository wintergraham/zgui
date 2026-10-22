#!/usr/bin/env python3
"""Refresh the Navbar lab's favicons, icons, logo tints and provenance from an Ask checkout."""
import argparse
import colorsys
import hashlib
import json
from pathlib import Path
import shutil

from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("source", type=Path, help="read-only Ask checkout (codex/react-native-mvp or later)")
args = parser.parse_args()
source = args.source.resolve()
react = source / "experiments/react"
target = Path(__file__).resolve().parents[1] / "crates/zgui-script/examples/navbar_lab"
assets = target / "assets"
assets.mkdir(parents=True, exist_ok=True)

# navbar.js SITES and EXTS: the bundled logos it draws.
FAVICONS = ["theverge.png", "github-mark.png", "wikipedia.png", "hackernews.png", "linear.png", "figma.png",
            "notion.png", "youtube.png", "google.png", "rtings.png", "reddit.png"]
for name in FAVICONS:
    shutil.copyfile(react / "public/assets/app/favicons" / name, assets / name)
shutil.copyfile(react / "public/assets/app/extensions/protonpass.png", assets / "protonpass.png")


def logo_tint(path):
    """navbar.js logoTint: saturation-weighted hue buckets over a 32px copy's opaque, non-grey pixels."""
    image = Image.open(path).convert("RGBA").resize((32, 32), Image.Resampling.BOX)  # closest to Chrome drawImage
    buckets = {}
    for r, g, b, a in image.get_flattened_data():
        h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
        if a / 255 < .5 or s < .25 or l < .12 or l > .9:
            continue
        e = buckets.setdefault(int(h * 12) % 12, [0, 0, 0, 0])
        w = s * a / 255
        e[0] += r * w; e[1] += g * w; e[2] += b * w; e[3] += w
    if not buckets:
        return None
    top = max(buckets.values(), key=lambda e: e[3])
    return "#" + "".join(f"{round(v / top[3]):02x}" for v in top[:3])


tints = {name: logo_tint(assets / name) for name in FAVICONS}

icon_path = react / "src/design/icons.ts"
text = icon_path.read_text()
icons = json.loads(text[text.index("{"):].rstrip().removesuffix(";"))
needed = ["app-window", "arrow-big-up", "arrow-down", "arrow-left", "arrow-right", "arrow-right-to-line", "arrow-up",
          "arrow-up-right", "bell", "book-open-text", "briefcase-business", "chevron-down", "chevron-right", "command",
          "cookie", "copy", "corner-down-left", "external-link", "eye", "folder", "globe", "history", "map-pin", "mic",
          "panels-top-left", "pin", "plus", "puzzle", "rotate-ccw", "rotate-cw", "search", "settings-2", "sparkles",
          "square", "star", "triangle-alert", "video", "volume-2", "wifi-off", "x", "lock", "check"]
assert set(needed) <= icons.keys(), sorted(set(needed) - icons.keys())
lines = ["-- Imported from Ask's design icons (experiments/react/src/design/icons.ts). See assets/Lucide-LICENSE.",
         "return {"]
lines += [f'    ["{name}"] = [=[{icons[name]}]=],' for name in needed]
lines += ["}", ""]
(target / "icons.luau").write_text("\n".join(lines))
shutil.copyfile(source / "assets/design-system/Lucide-LICENSE", assets / "Lucide-LICENSE")

lines = ["-- Dominant logo colours (navbar.js logoTint), computed by scripts/import_navbar.py.", "return {"]
lines += [f'    ["{name}"] = "{tint}",' for name, tint in tints.items() if tint]
lines += ["}", ""]
(target / "tints.luau").write_text("\n".join(lines))

inputs = ["experiments/react/src/pages/NavbarPage.tsx", "experiments/react/src/controls/navbar.js",
          "experiments/react/src/design/navbar.css", "experiments/react/src/design/dither.js",
          "experiments/react/src/design/tokens.css", "experiments/react/src/design/pages.css",
          "experiments/react/src/controls/behavior.js", "experiments/react/src/design/icons.ts"]
manifest = {"source_branch": "codex/react-native-mvp", "source_inputs_sha256": {
    name: hashlib.sha256((source / name).read_bytes()).hexdigest() for name in inputs
}, "logo_tints": tints}
(target / "source.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"Imported {len(FAVICONS) + 1} logos, {len(needed)} icons and source checksums into {target}")
