"""Build every tutorial's timeline from the approved template (1.03) and export them all into final/.

    python batch_template.py            # everything
    python batch_template.py t04-get-latest s01-server   # just these

Each tutorial runs in its own process with a time limit, so one stuck Kitlig call cannot hold up the rest.
A tutorial that already has its numbered timeline built from the template (1.01 to 1.03) is only exported.
Logs to ../out/template-log.txt.
"""
import json
import os
import re
import shutil
import subprocess
import sys

import assemble as a
from from_template import ORDER, TEMPLATE, number

HERE = os.path.dirname(os.path.abspath(__file__))
FINAL = os.path.join(HERE, "..", "final")
KITLIG_PROJECT = r"P:\Projects\UAI\MultiParity\Kitlig\multiparity"
LOG = os.path.join(HERE, "..", "out", "template-log.txt")
LIMIT = 1500


def log(msg):
    print(msg, flush=True)
    with open(LOG, "a", encoding="utf8") as f:
        f.write(msg + "\n")


def final_name(sid):
    return sid.split("-")[0] + " " + re.sub(r"[^A-Za-z0-9 ,.-]", "", a.TITLES[sid]) + ".mp4"


def built_from_template(sid):
    """The numbered timeline for sid if it was made from the template (it has the BG_Music lane)."""
    for s in a.call("list_sequences", {}):
        if s["name"] == number(sid):
            q = a.call("get_sequence", {"sequenceId": s["sequenceId"]})
            if any(t["name"] == "BG_Music" for t in q["tracks"]):
                return s["sequenceId"]
            # An older cut carrying the number: give the number back so the new one can have it.
            a.call("rename_sequence", {"sequenceId": s["sequenceId"], "name": f"MultiParity · {a.TITLES[sid]} (old)"})
    return None


def export(sid, seq):
    code = f"import assemble as a, json; print(json.dumps(a.call('export_sequence', {{'sequenceId': '{seq}', 'codec': 'h264', 'waitSeconds': 1200}})))"
    r = subprocess.run([sys.executable, "-c", code], cwd=HERE, capture_output=True, text=True, timeout=LIMIT)
    line = next((l for l in r.stdout.splitlines() if '"outputs"' in l), None)
    if not line:
        raise RuntimeError((r.stdout + r.stderr)[-600:])
    path = json.loads(line)["outputs"][0]["path"]
    shutil.copyfile(os.path.join(KITLIG_PROJECT, path), os.path.join(FINAL, final_name(sid)))


def main(sids):
    for sid in sids:
        try:
            seq = built_from_template(sid)
            if not seq:
                r = subprocess.run([sys.executable, "from_template.py", sid], cwd=HERE, capture_output=True, text=True, timeout=LIMIT)
                line = next((l for l in r.stdout.splitlines() if l.startswith("{")), None)
                if not line:
                    raise RuntimeError((r.stdout + r.stderr)[-600:])
                seq = json.loads(line)["sequenceId"]
            if BUILD_ONLY:
                log(f"BUILT {number(sid)} {seq}")
                continue
            export(sid, seq)
            log(f"OK {number(sid)} -> final/{final_name(sid)}")
        except Exception as e:  # noqa: BLE001 - keep going, report at the end
            log(f"FAIL {sid}: {str(e)[-400:]}")


BUILD_ONLY = "--build-only" in sys.argv

if __name__ == "__main__":
    main([s for s in sys.argv[1:] if not s.startswith("--")] or ORDER)
