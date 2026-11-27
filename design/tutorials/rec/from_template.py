"""Make a tutorial's timeline from the approved template cut (your mix: music bed, UI sound effects on the
cards, the longer end card), swapping in this tutorial's recording, voice lines, callouts and title.

    python from_template.py t01-setup [--export]

The template is "1.03 Your first publish". Its opening sound effects stay where they are; the end card and
the sound effects under it move to this tutorial's end. The music is the full-length version of the
template's track, placed so it ends where the end card ends (as the 60-second edit does), faded in at 0.
"""
import json
import os
import shutil
import sys

import assemble as a

TEMPLATE = os.environ.get("MP_TEMPLATE_SEQ", "rdseq_01M8E2R9Y3EZF1CWN3YD2G0BEE")
MUSIC_FULL = os.path.join(r"P:\Projects\UAI\MultiParity\Kitlig\multiparity\audio\source", "tokyo-and-you_main-full.wav")
MUSIC_FULL_SEC = 153.6
MUSIC_SILENT_AT = 150.015   # where the full track's last note has died away (ffmpeg silencedetect, -45 dB)
END_BEFORE_MUSIC = 0.37     # the end card finishes this long before the music does
ORDER = sorted(a.TITLES, key=lambda s: (s[0] != "t", s))


def number(sid):
    series = "1" if sid.startswith("t") else "2"
    return f"{series}.{sid[1:3]} {a.TITLES[sid]}"


def track_named(s, name):
    return next(t["trackId"] for t in s["tracks"] if t["name"] == name)


def main(sid, export):
    tpl = a.call("get_sequence", {"sequenceId": TEMPLATE})
    end_tpl = next(c for c in tpl["clips"] if c["label"].startswith("end-"))
    end_start_tpl, end_dur = end_tpl["startSec"], end_tpl["durationSec"]

    seq = a.call("duplicate_sequence", {"sequenceId": TEMPLATE})
    seq = seq.get("sequenceId") if isinstance(seq, dict) else str(seq).strip()
    a.call("rename_sequence", {"sequenceId": seq, "name": number(sid)})
    s = a.call("get_sequence", {"sequenceId": seq})
    v1, a1, callouts, cards, music = (track_named(s, n) for n in ("V1", "A1", "Callouts", "Title", "BG_Music"))
    # The copy's music ducker still listens to the template's voice lane: point it at this one's A1.
    a.call("set_track_mix", {"sequenceId": seq, "trackId": music, "duckUnderTrackIds": [a1]})

    # Out: the template's recording, voice, callouts, intro title and music.
    out = [c["clipId"] for c in s["clips"] if c["trackId"] in (v1, a1, callouts, music) or c["label"].startswith("intro-")]
    a.call("remove_clips", {"sequenceId": seq, "clipIds": out})
    # What sits on the end card (the card and its sound effects), noted before this tutorial's clips go in.
    at_end = [(c["clipId"], c["startSec"]) for c in s["clips"] if c["clipId"] not in out and c["startSec"] >= end_start_tpl - 0.25]

    d = os.path.join(a.OUT, sid)
    tl = json.load(open(os.path.join(d, "timeline.json"), encoding="utf8"))
    lines = {l["id"]: l for l in a.LINES[sid]}
    src = os.path.abspath(os.path.join(d, f"{sid}.mp4"))
    shutil.copyfile(os.path.join(d, "video.mp4"), src)
    vid = a.first_id(a.call("import_files", {"paths": [src], "folder": "video/tutorials"}))
    a.call("add_media_clip", {"sequenceId": seq, "assetId": vid, "trackId": v1, "startSec": a.LEAD, "streams": "video"})
    for b in tl["beats"]:
        a.call("add_media_clip", {"sequenceId": seq, "assetId": lines[b["id"]]["assetId"], "trackId": a1,
                                  "startSec": round(a.LEAD + b["t"] + 0.05, 3), "streams": "audio"})
    for c in tl["callouts"]:
        a.callout_clip(seq, callouts, dict(c, t=c["t"] + a.LEAD))
    a.card_clip(seq, cards, a.INTRO_CARD, a.CARDS[a.LOGO_CARD]["dur"], {"title": a.TITLES[sid]})

    # The end card and everything under it move to this tutorial's end.
    end_start = round(a.LEAD + tl["duration"] - a.END_OVERLAP, 3)
    shift = end_start - end_start_tpl
    moves = [{"clipId": cid, "startSec": round(t + shift, 3)} for cid, t in at_end]
    a.call("move_clips", {"sequenceId": seq, "moves": moves, "unlinked": True})

    total = end_start + end_dur
    assets = a.call("import_files", {"paths": [MUSIC_FULL], "folder": "audio/source"})
    mus = a.first_id(assets)
    head = max(0.0, MUSIC_FULL_SEC - total)
    r = a.call("add_media_clip", {"sequenceId": seq, "assetId": mus, "trackId": music, "startSec": 0,
                                  "sourceStartSec": round(head, 3), "durationSec": round(min(total, MUSIC_FULL_SEC), 3), "streams": "audio"})
    mclip = r.get("clipId") if isinstance(r, dict) else None
    if not mclip:
        mclip = next(c["clipId"] for c in a.call("get_sequence", {"sequenceId": seq})["clips"] if c["trackId"] == music)
    a.call("set_clip_fade", {"sequenceId": seq, "clipIds": [mclip], "inSec": 1.5})
    # The full track goes silent at MUSIC_SILENT_AT, before the file ends: the music stops there, and the
    # end card stops a beat before it (as the hand-finished timelines do).
    music_end = round(total - (MUSIC_FULL_SEC - MUSIC_SILENT_AT), 3)
    a.call("trim_edit_clip", {"sequenceId": seq, "clipId": mclip, "startSec": 0, "endSec": music_end})
    endc = next(c for c in a.call("get_sequence", {"sequenceId": seq})["clips"] if c["label"].startswith("end-"))
    a.call("trim_edit_clip", {"sequenceId": seq, "clipId": endc["clipId"], "startSec": endc["startSec"], "endSec": round(music_end - END_BEFORE_MUSIC, 3)})
    total = music_end
    print(json.dumps({"sequenceId": seq, "name": number(sid), "total": round(total, 2), "musicFrom": round(head, 2)}))
    if export:
        r = a.call("export_sequence", {"sequenceId": seq, "codec": "h264", "waitSeconds": 900})
        print(json.dumps(r)[:600])


if __name__ == "__main__":
    main(sys.argv[1], "--export" in sys.argv)
