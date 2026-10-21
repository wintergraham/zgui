#!/usr/bin/env python3
"""Refresh Controls fonts/icons and provenance from the current Ask checkout."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("source", type=Path, help="read-only Ask checkout, codex/native-react-validation")
args = parser.parse_args()
source = args.source.resolve()
target = Path(__file__).resolve().parents[1] / "crates/zgui-script/examples/controls_lab"
assets = target / "assets"
assets.mkdir(parents=True, exist_ok=True)
for name in ["Geist.ttf", "GeistMono.ttf", "Geist-OFL.txt", "GeistMono-OFL.txt", "Lucide-LICENSE"]:
    shutil.copyfile(source / "assets/design-system" / name, assets / name)
    if name.endswith(".txt"):
        path = assets / name
        path.write_text("\n".join(line.rstrip() for line in path.read_text().splitlines()) + "\n")

icon_path = "experiments/react/src/design/icons.ts"
text = (source / icon_path).read_text()
icons = json.loads(text[text.index("{"):].rstrip().removesuffix(";"))
# These two glyphs are registered by behavior.js rather than design/icons.ts.
feather = {
    "droplet": '<path d="M12 2.69l5.66 5.66a8 8 0 1 1-11.31 0z"/>',
    "mouse-pointer": '<path d="M3 3l7.07 16.97 2.51-7.39 7.39-2.51L3 3z"/><path d="M13 13l6 6"/>',
}
for name, paths in feather.items():
    icons[name] = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' + paths + '</svg>'
needed = ["plus", "ellipsis", "pin", "search", "check", "minus", "x", "chevrons-up-down", "chevron-right", "copy", "folder-input", "circle-alert", "corner-down-left", "command", "sliders-horizontal", "droplet", "git-branch", "lock", "message-circle", "mouse-pointer", "panel-left", "panel-top", "message-square", "history", "trash-2", "settings", "component"]
assert set(needed) <= icons.keys(), "source is missing Controls icons"
lines = ["-- Imported from codex/native-react-validation Ask design icons. See assets/Lucide-LICENSE.", "return {"]
lines += [f'  ["{name}"] = [=[{icons[name]}]=],' for name in needed]
lines += ["}", ""]
(target / "icons.luau").write_text("\n".join(lines))

inputs = ["experiments/react/src/pages/ControlsPage.tsx", "experiments/react/src/controls/Preset.tsx",
          "experiments/react/src/controls/behavior.js", "experiments/react/src/controls/SettingsKit.tsx",
          "experiments/react/src/controls/CommandPalette.tsx", "experiments/react/src/design/tokens.css", icon_path]
manifest = {"source_branch": "codex/native-react-validation", "source_inputs_sha256": {
    name: hashlib.sha256((source / name).read_bytes()).hexdigest() for name in inputs
}}
(target / "source.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"Imported {len(needed)} icons, two fonts, licenses, and source checksums into {target}")
