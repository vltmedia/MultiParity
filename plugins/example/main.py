"""Example MultiParity plugin.

MultiParity starts this program for each hook or operation, writes one JSON request to stdin,
and reads JSON lines from stdout:

    {"message": "Working on 3 of 10"}      progress, shown in the app (any number of these)
    {"result": {"ok": true, "message": "..."}}   the answer (last)
    {"error": "what went wrong"}           instead of a result

For a before_ hook, {"result": {"ok": false, "message": "why"}} stops what was about to happen.
Only the standard library is used, so it runs wherever Python 3 does.
"""
import json
import os
import struct
import sys
from collections import Counter


def say(**kw):
    print(json.dumps(kw), flush=True)


def before_publish(req):
    # A naming rule: no spaces in the names of new or edited files.
    bad = [f["path"] for f in req["data"]["files"] if f["op"] != "delete" and " " in f["path"].split("/")[-1]]
    if bad:
        shown = ", ".join(bad[:5]) + (f" and {len(bad) - 5} more" if len(bad) > 5 else "")
        return {"ok": False, "message": f"File names must not have spaces: {shown}. Rename them and publish again."}
    return {"ok": True}


def after_publish(req):
    d = req["data"]
    return {"ok": True, "message": f"Change {d['change']} published with {len(d['paths'])} file(s)."}


def image_size(path):
    with open(path, "rb") as f:
        head = f.read(26)
        if head[:8] == b"\x89PNG\r\n\x1a\n":
            w, h = struct.unpack(">II", head[16:24])
            return w, h
        if head[:2] == b"\xff\xd8":
            f.seek(2)
            while True:
                marker, size = struct.unpack(">HH", f.read(4))
                if 0xFFC0 <= marker <= 0xFFCF and marker not in (0xFFC4, 0xFFC8, 0xFFCC):
                    h, w = struct.unpack(">xHH", f.read(5))
                    return w, h
                f.seek(size - 2, 1)
    raise ValueError("not a PNG or JPEG")


def op_image_size(req):
    out = []
    for i, f in enumerate(req["files"], 1):
        say(message=f"Reading {i} of {len(req['files'])}")
        try:
            w, h = image_size(f["abs"])
            out.append(f"{f['path'].split('/')[-1]}: {w} x {h}")
        except Exception as e:  # noqa: BLE001 - report it, keep going
            out.append(f"{f['path'].split('/')[-1]}: {e}")
    return {"ok": True, "message": "; ".join(out)}


def op_count_types(req):
    root = req["workspace"]["root"]
    kinds = Counter()
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d != ".mp"]
        for n in filenames:
            kinds[os.path.splitext(n)[1].lower() or "(none)"] += 1
    top = ", ".join(f"{n} {ext}" for ext, n in kinds.most_common(6))
    return {"ok": True, "message": f"{sum(kinds.values())} files: {top}.", "counts": dict(kinds)}


def main():
    req = json.loads(sys.stdin.readline())
    if req["kind"] == "hook":
        handler = {"before_publish": before_publish, "after_publish": after_publish}.get(req["hook"])
    else:
        handler = {"image-size": op_image_size, "count-types": op_count_types}.get(req["operation"])
    if handler is None:
        say(error=f"This plugin does not handle {req.get('hook') or req.get('operation')}.")
        return
    say(result=handler(req))


if __name__ == "__main__":
    main()
