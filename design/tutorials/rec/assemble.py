"""Cut a recorded tutorial on a Kitlig timeline: the screen recording on V1, each voice line at its
beat on A1, the GLSL callouts on V2 and the title on V3. Then export it.

    python assemble.py t01-setup [--export]

Needs Kitlig running with its MCP server, and the scratch client kitlig.py (KITLIG_CLIENT dir)."""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.environ.get("KITLIG_CLIENT", r"P:\temp\Claude\claude\7df7f3888d1632d5\a22ff03a-1f55-4c9c-8b8f-318966a068cc\scratchpad"))
import kitlig  # noqa: E402

OUT = os.path.join(HERE, "..", "out")
LINES = json.load(open(os.path.join(HERE, "..", "vo-lines.json"), encoding="utf8"))
GLSL = {n: open(os.path.join(HERE, "glsl", n + ".glsl"), encoding="utf8").read() for n in ["common", "ring", "spot", "sweep", "tag"]}
ACCENT = "#26caf3"
FW, FH = 1920, 1080
TITLES = {
    "t01-setup": "Getting set up", "t02-new-project": "Starting a brand-new project", "t03-first-publish": "Your first publish",
    "t04-get-latest": "Staying up to date", "t05-claims": "Who's working on what", "t06-checkpoints": "Checkpoints",
    "t07-versions": "Going back to an older version", "t08-drafts": "Drafts", "t09-branches": "Branches",
    "t10-web-app": "The web app", "t11-admin": "For admins", "s01-server": "A server in five minutes",
    "s02-storage": "Where the files live", "s03-remote": "Working from anywhere", "s04-backups": "Backups",
    "s05-email": "Email", "s06-updating": "Updating", "s07-pipeline": "Hooking it into your pipeline",
    "s08-cli": "Build machines and the command line",
    "t12-your-work": "Finding what you changed", "t13-size-map": "Seeing what takes up space", "t14-export": "Exporting a project",
    "t15-clients": "Clients and productions", "t16-sync-production": "Syncing a whole production", "t17-policies": "Who can see what",
    "t18-plugins": "Plugins", "t19-ai": "AI assistants",
}


def call(name, args):
    r = kitlig.rpc("tools/call", {"name": name, "arguments": args})
    if not r or "error" in r:
        raise RuntimeError(f"{name}: {r}")
    res = r["result"]
    txt = "".join(c.get("text", "") for c in res.get("content", []) if c.get("type") == "text")
    if res.get("isError"):
        raise RuntimeError(f"{name}: {txt}")
    try:
        return json.loads(txt)
    except ValueError:
        return txt


def box(rect):
    """Recorder rect (output pixels, top-left origin) -> uv min/max from the bottom left."""
    x0, x1 = max(0.0, rect["x"] / FW), min(1.0, (rect["x"] + rect["w"]) / FW)
    y0, y1 = max(0.0, 1 - (rect["y"] + rect["h"]) / FH), min(1.0, 1 - rect["y"] / FH)
    return [round(x0, 4), round(y0, 4)], [round(x1, 4), round(y1, 4)]


def callout_clip(seq, track, c):
    if c["style"] == "code":  # a terminal or settings line: drawn as a tag
        c = dict(c, style="tag")
    bmin, bmax = box(c["rect"])
    inputs = [
        {"name": "boxMin", "type": "vec2", "value": bmin},
        {"name": "boxMax", "type": "vec2", "value": bmax},
        {"name": "accent", "type": "color", "value": ACCENT},
    ]
    style = c["style"]
    if style == "tag" or (c.get("label") and style != "tag"):
        # The label goes right of the target when there is room, otherwise left; level with it.
        room_right = 1 - bmax[0]
        size = 2.4  # % of frame height
        width = (len(c["label"]) * 0.56 * size / 100 * FH) / FW
        right = room_right > width + 0.05
        x = bmax[0] + 0.022 if right else bmin[0] - 0.022 - width
        y = (bmin[1] + bmax[1]) / 2
        inputs.append({"name": "label", "type": "text", "value": c["label"], "fontId": "space-grotesk", "sizePct": size, "bold": True,
                       "align": "left", "x": round(max(0.01, x), 4), "y": round(y, 4)})
    if style != "tag" and c.get("label"):
        # A spot (or ring) with words: draw the spot, and a tag on top of it.
        callout_clip(seq, track, dict(c, label=""))
        style = "tag"
    src = GLSL["common"] + "\n" + GLSL[style]
    return call("add_glsl_clip", {"sequenceId": seq, "trackId": track, "startSec": round(c["t"], 3), "durationSec": round(c["dur"], 3),
                                  "label": f"{style}: {c.get('label') or ''}".strip(": "), "source": src, "inputs": inputs})


def first_id(x):
    """The first asset id anywhere in a tool result."""
    if isinstance(x, dict):
        for k in ("assetId", "id"):
            if isinstance(x.get(k), str) and x[k].startswith("asset_"):
                return x[k]
        x = list(x.values())
    if isinstance(x, list):
        for v in x:
            r = first_id(v)
            if r:
                return r
    return None


def find_track(s, kind):
    for t in s.get("tracks", []):
        if (t.get("kind") == "video") == (kind == "video"):
            return t["trackId"]


def main(sid, export):
    d = os.path.join(OUT, sid)
    tl = json.load(open(os.path.join(d, "timeline.json"), encoding="utf8"))
    lines = {l["id"]: l for l in LINES[sid]}
    import shutil
    src = os.path.abspath(os.path.join(d, f"{sid}.mp4"))
    shutil.copyfile(os.path.join(d, "video.mp4"), src)
    res = call("import_files", {"paths": [src], "folder": "video/tutorials"})
    vid = first_id(res)
    name = f"MultiParity · {TITLES[sid]}"
    # A re-cut gets its own name; earlier cuts are left for you to compare or delete.
    taken = {q.get("name") for q in call("list_sequences", {})}
    n = 2
    base = name
    while name in taken:
        name, n = f"{base} (cut {n})", n + 1
    seq = call("create_edit_sequence", {"name": name, "fps": 30, "frame": {"width": FW, "height": FH}})
    seq = seq.get("sequenceId") if isinstance(seq, dict) else seq
    s = call("get_sequence", {"sequenceId": seq})
    v1, a1 = find_track(s, "video"), find_track(s, "audio")
    call("add_media_clip", {"sequenceId": seq, "assetId": vid, "trackId": v1, "startSec": 0, "streams": "video"})
    for b in tl["beats"]:
        l = lines[b["id"]]
        call("add_media_clip", {"sequenceId": seq, "assetId": l["assetId"], "trackId": a1, "startSec": round(b["t"] + 0.05, 3), "streams": "audio"})
    v2 = call("add_track", {"sequenceId": seq, "kind": "video", "name": "Callouts"})
    v2 = v2.get("trackId") if isinstance(v2, dict) else v2.strip()
    for c in tl["callouts"]:
        callout_clip(seq, v2, c)
    v3 = call("add_track", {"sequenceId": seq, "kind": "video", "name": "Title"})
    v3 = v3.get("trackId") if isinstance(v3, dict) else v3.strip()
    call("add_title_clip", {"sequenceId": seq, "trackId": v3, "startSec": 0.3, "durationSec": 3.6, "presetId": "lower-third-1",
                            "lines": [{"role": "header", "text": "MultiParity tutorial", "sizePct": 2.4, "color": "#26caf3"},
                                      {"role": "body", "text": TITLES[sid], "sizePct": 5.2, "bold": True, "color": "#eef6fb"}],
                            "fontId": "space-grotesk", "bar": {"color": "#071e2e", "opacity": 0.88}})
    print(json.dumps({"sequenceId": seq, "callouts": len(tl["callouts"]), "beats": len(tl["beats"])}))
    if export:
        r = call("export_sequence", {"sequenceId": seq, "codec": "h264", "waitSeconds": 900})
        print(json.dumps(r)[:600])


if __name__ == "__main__":
    main(sys.argv[1], "--export" in sys.argv)
