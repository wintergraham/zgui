#!/usr/bin/env python3
"""A test-owned drag source for drop probes: a GTK 3 window whose button
drags TEXT as another app's selection does (GTK's own XDND on X11, its
`wl_data_device` on Wayland with `GDK_BACKEND=wayland`), offered as UTF-8
text in the formats browsers and editors offer.

    text_drag_source.py TITLE X Y TEXT

Prints `READY` once shown, `SENT <target>` as a drop asks for the text,
`FAILED` when a drag is let go of where nothing takes it (or cancelled), and
`END` as each drag ends."""
import argparse
import sys

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, Gtk  # noqa: E402

FORMATS = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("title")
    parser.add_argument("x", type=int)
    parser.add_argument("y", type=int)
    parser.add_argument("text")
    args = parser.parse_args()

    window = Gtk.Window(title=args.title)
    window.set_default_size(240, 120)
    window.move(args.x, args.y)
    window.connect("destroy", Gtk.main_quit)
    button = Gtk.Button(label="Drag text")
    window.add(button)
    targets = [Gtk.TargetEntry.new(name, 0, index) for index, name in enumerate(FORMATS)]
    button.drag_source_set(Gdk.ModifierType.BUTTON1_MASK, targets, Gdk.DragAction.COPY)

    def data(widget, context, selection, info, time):
        selection.set_text(args.text, -1)
        print("SENT", FORMATS[info], flush=True)

    def failed(widget, context, result):
        print("FAILED", int(result), flush=True)
        return True

    button.connect("drag-data-get", data)
    button.connect("drag-failed", failed)
    button.connect("drag-end", lambda *_: print("END", flush=True))
    window.show_all()
    print("READY", flush=True)
    Gtk.main()


if __name__ == "__main__":
    sys.exit(main())
