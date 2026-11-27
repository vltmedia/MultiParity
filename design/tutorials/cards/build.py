"""Build gallery.html: every card shader from intros/logos/ends.glsl, playing live, for choosing looks.

    python build.py [frame.jpg]

The optional picture goes under the cards (a frame of a tutorial). cards.json is written too: each card's
id, name, length, inputs and source, ready for add_glsl_clip (with mp.glsl in front of the source).
"""
import base64
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def cards():
    out = []
    for f in ["logos.glsl", "intros.glsl", "ends.glsl"]:
        text = open(os.path.join(HERE, f), encoding="utf8").read()
        parts = re.split(r"^// @card ", text, flags=re.M)[1:]
        for part in parts:
            head, rest = part.split("\n", 1)
            cid, name, dur = [s.strip() for s in head.split("|")]
            m = re.match(r"// @inputs (.*)\n", rest)
            inputs = json.loads(m.group(1))
            rest = rest[m.end():]
            desc = ""
            if rest.startswith("// "):
                line, rest = rest.split("\n", 1)
                desc = line[3:].strip()
            out.append({"id": cid, "name": name, "dur": float(dur), "desc": desc, "inputs": inputs, "src": rest.strip() + "\n"})
    return out


def main():
    cs = cards()
    mp = open(os.path.join(HERE, "mp.glsl"), encoding="utf8").read()
    bg = ""
    if len(sys.argv) > 1:
        bg = "data:image/jpeg;base64," + base64.b64encode(open(sys.argv[1], "rb").read()).decode()
    html = open(os.path.join(HERE, "gallery.src.html"), encoding="utf8").read()
    safe = lambda v: json.dumps(v).replace("<", "\\u003c")
    html = html.replace('/*MP*/""', safe(mp)).replace("/*CARDS*/[]", safe(cs)).replace('/*BG*/""', safe(bg))
    open(os.path.join(HERE, "gallery.html"), "w", encoding="utf8").write(html)
    json.dump([{k: c[k] for k in ("id", "name", "dur", "desc", "inputs", "src")} for c in cs],
              open(os.path.join(HERE, "cards.json"), "w", encoding="utf8"), indent=1)
    print(f"{len(cs)} cards -> gallery.html")


if __name__ == "__main__":
    main()
