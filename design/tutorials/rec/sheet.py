"""sheet.py <scene> [video]: a contact sheet of a recording, one frame a little into each beat
(and one near the end), for checking a take at a glance. Writes out/<scene>/sheet.jpg."""
import json, os, subprocess, sys
from PIL import Image

sid = sys.argv[1]
d = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "out", sid)
video = sys.argv[2] if len(sys.argv) > 2 else os.path.join(d, "video.mp4")
tl = json.load(open(os.path.join(d, "timeline.json")))
times = [b["t"] + 2.2 for b in tl["beats"]] + [tl["duration"] - 1.0]
ims = []
for i, t in enumerate(times):
    f = os.path.join(d, f"_s{i}.jpg")
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", f"{t:.2f}", "-i", video, "-frames:v", "1", "-vf", "scale=640:-1", f], check=True)
    ims.append(Image.open(f))
cols = 3
w, h = ims[0].size
g = Image.new("RGB", (w * cols, h * ((len(ims) + cols - 1) // cols)))
for i, im in enumerate(ims):
    g.paste(im, ((i % cols) * w, (i // cols) * h))
out = os.path.join(d, "sheet.jpg")
g.save(out, quality=85)
for i in range(len(ims)):
    os.remove(os.path.join(d, f"_s{i}.jpg"))
print(out)
