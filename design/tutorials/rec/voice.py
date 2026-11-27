"""Generate voice lines in Kitlig (fal ElevenLabs v3, the tutorials' voice) for scenes in a lines file,
and add them, with their exact lengths, to vo-lines.json. Lines already there are kept.

    python voice.py ../new-lines.json [scene ...]
"""
import json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.environ.get("KITLIG_CLIENT", r"P:\temp\Claude\claude\7df7f3888d1632d5\a22ff03a-1f55-4c9c-8b8f-318966a068cc\scratchpad"))
import kitlig  # noqa: E402

VOICE = "5OhY6eII5od0GRAOSINf"
KITLIG_PROJECT = os.environ.get("KITLIG_PROJECT", r"P:\Projects\UAI\MultiParity\Kitlig\multiparity")
VO_PATH = os.path.join(HERE, "..", "vo-lines.json")


def speak(text):
    r = kitlig.rpc("tools/call", {"name": "run_operation", "arguments": {
        "pipelineId": "audio.elevenlabs-tts-v3", "inputs": {"text": text, "voice": VOICE, "stability": 0.5}, "waitSeconds": 240}})
    res = r["result"]
    j = json.loads("".join(c.get("text", "") for c in res.get("content", []) if c.get("type") == "text"))
    if j.get("status") != "complete":
        raise RuntimeError(f"{j.get('status')}: {j.get('error')}")
    out = j["outputs"][0]
    return out["assetId"], out["path"]


def length(path):
    p = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", os.path.join(KITLIG_PROJECT, path)],
                       capture_output=True, text=True)
    return round(float(p.stdout.strip()), 3)


def save(vo):
    # In one step: the recorder may be reading it.
    tmp = VO_PATH + ".tmp"
    json.dump(vo, open(tmp, "w", encoding="utf8"), indent=1, ensure_ascii=False)
    os.replace(tmp, VO_PATH)


def main(src, only):
    new = json.load(open(src, encoding="utf8"))
    vo = json.load(open(VO_PATH, encoding="utf8"))
    for sid, lines in new.items():
        if only and sid not in only:
            continue
        have = {l["id"]: l for l in vo.get(sid, [])}
        out = []
        for l in lines:
            old = have.get(l["id"])
            if old and old.get("text") == l["text"] and old.get("assetId"):
                out.append(old)
                continue
            asset, path = speak(l["text"])
            out.append({"id": l["id"], "text": l["text"], "assetId": asset, "path": path, "dur": length(path)})
            print(f"{sid}/{l['id']}: {out[-1]['dur']} s", flush=True)
            vo[sid] = out
            save(vo)
        vo[sid] = out
        save(vo)


if __name__ == "__main__":
    main(sys.argv[1], set(sys.argv[2:]))
