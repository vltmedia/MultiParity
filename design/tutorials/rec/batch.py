"""Record, cut and export tutorials one after another: the screen recording (record.mjs), a
contact sheet, the Kitlig cut and export (assemble.py), then a copy in final/.

    python batch.py t01-setup t02-new-project ...

Each result is appended to out/batch-log.txt; a failed scene is logged and the next one runs."""
import json, os, re, shutil, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "out")
FINAL = os.path.join(HERE, "..", "final")
KITLIG_PROJECT = os.environ.get("KITLIG_PROJECT", r"P:\Projects\UAI\MultiParity\Kitlig\multiparity")
sys.path.insert(0, HERE)
from assemble import TITLES  # noqa: E402


def log(msg):
    line = f"{time.strftime('%H:%M:%S')} {msg}"
    print(line, flush=True)
    with open(os.path.join(OUT, "batch-log.txt"), "a", encoding="utf8") as f:
        f.write(line + "\n")


def run(cmd, timeout):
    p = subprocess.run(cmd, cwd=HERE, capture_output=True, text=True, encoding="utf8", errors="replace", timeout=timeout)
    if p.returncode != 0:
        raise RuntimeError((p.stdout + p.stderr).strip()[-1200:])
    return p.stdout


def one(sid):
    d = os.path.join(OUT, sid)
    os.makedirs(d, exist_ok=True)
    rec = run(["node", "record.mjs", f"scenes/{sid}.mjs", d], 900).strip().splitlines()[-1]
    run([sys.executable, "sheet.py", sid], 300)
    out = run([sys.executable, "assemble.py", sid, "--export"], 1800)
    exported = None
    for line in out.splitlines():
        if '"outputs"' in line:
            j = json.loads(line)
            exported = j["outputs"][0]["path"]
    if not exported:
        raise RuntimeError("Kitlig did not export it: " + out[-600:])
    name = f"{sid.split('-')[0]} {re.sub(r'[^A-Za-z0-9 ,.-]', '', TITLES[sid])}.mp4"
    shutil.copyfile(os.path.join(KITLIG_PROJECT, exported), os.path.join(FINAL, name))
    return rec, name


if __name__ == "__main__":
    for sid in sys.argv[1:]:
        try:
            rec, name = one(sid)
            log(f"OK   {sid}: {rec} -> final/{name}")
        except Exception as e:  # noqa: BLE001 - log it and go on to the next scene
            # The line that says what went wrong (Node's own last line is just its version).
            msg = [l for l in str(e).splitlines() if "Error" in l or "Not found" in l or "No voice" in l]
            log(f"FAIL {sid}: {(msg[0] if msg else str(e).splitlines()[-1]).strip()[:300]}")
