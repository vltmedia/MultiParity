# MultiParity for Blender: see and change the open file's state without leaving Blender.
#
# Talks to the MultiParity app on this computer (the desktop app, or `mp daemon`), so it uses
# the account and synced folder already set up there. Nothing is sent anywhere else.
#
# Install: Edit > Preferences > Add-ons > Install from Disk, choose multiparity.zip (or this
# folder zipped), and tick "MultiParity". The panel is in the 3D view's sidebar (N), tab
# "MultiParity".

bl_info = {
    "name": "MultiParity",
    "author": "MultiParity",
    "version": (0, 1, 0),
    "blender": (2, 93, 0),
    "location": "3D View > Sidebar > MultiParity",
    "description": "Claim, publish, checkpoint and get latest for the open file with MultiParity",
    "category": "System",
}

import json
import urllib.error
import urllib.parse
import urllib.request

import bpy
from bpy.app.handlers import persistent

DEFAULT_APP = "http://127.0.0.1:18519"

# The open file's state, as the app last reported it.
_state = {"info": None, "error": None, "versions": []}


def _app_url():
    try:
        prefs = bpy.context.preferences.addons[__name__].preferences
        return prefs.app_url.rstrip("/")
    except (KeyError, AttributeError):
        return DEFAULT_APP


def call(method, path, body=None, timeout=60):
    """Call the MultiParity app. Returns the JSON answer; raises RuntimeError with its message."""
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(_app_url() + "/local" + path, data=data, method=method, headers={"content-type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        try:
            msg = json.loads(e.read())["error"]["message"]
        except Exception:
            msg = "The MultiParity app answered %s." % e.code
        raise RuntimeError(msg)
    except (urllib.error.URLError, OSError):
        raise RuntimeError("The MultiParity app is not running on this computer. Start it, then press Refresh.")


def refresh():
    path = bpy.data.filepath
    _state["versions"] = []
    if not path:
        _state["info"], _state["error"] = None, None
        return
    try:
        _state["info"] = call("GET", "/file?path=" + urllib.parse.quote(path), timeout=10)
        _state["error"] = None
    except RuntimeError as e:
        _state["info"], _state["error"] = None, str(e)
    for area in getattr(bpy.context.screen, "areas", []):
        area.tag_redraw()


def _ws():
    i = _state["info"] or {}
    return i.get("workspace_id"), i.get("path"), i.get("branch_id")


def _server(path, body):
    ws, _, _ = _ws()
    return call("POST", "/workspaces/%s/server/%s" % (ws, path), body)


class MP_Prefs(bpy.types.AddonPreferences):
    bl_idname = __name__
    app_url: bpy.props.StringProperty(name="MultiParity app", default=DEFAULT_APP, description="Where the MultiParity app on this computer listens")

    def draw(self, context):
        self.layout.prop(self, "app_url")


class MP_OT_refresh(bpy.types.Operator):
    bl_idname = "multiparity.refresh"
    bl_label = "Refresh"
    bl_description = "Ask MultiParity about this file again"

    def execute(self, context):
        refresh()
        return {"FINISHED"}


class _Action(bpy.types.Operator):
    """Shared: run, report the result, refresh."""

    def run(self, context):
        raise NotImplementedError

    def execute(self, context):
        try:
            msg = self.run(context)
            if msg:
                self.report({"INFO"}, msg)
        except RuntimeError as e:
            self.report({"ERROR"}, str(e))
        refresh()
        return {"FINISHED"}


class MP_OT_claim(_Action):
    bl_idname = "multiparity.claim"
    bl_label = "Claim"
    bl_description = "Claim this file so nobody else can publish it while you work"

    def run(self, context):
        _, rel, branch = _ws()
        r = _server("claims", {"branch": branch, "paths": [rel], "device": "Blender"})
        held = r.get("held") or []
        if held:
            raise RuntimeError("%s has it claimed. Ask them to release it." % ((held[0].get("user") or {}).get("name") or "Someone"))
        return "Claimed. Nobody else can publish it until you publish or release."


class MP_OT_release(_Action):
    bl_idname = "multiparity.release"
    bl_label = "Release"
    bl_description = "Give up your claim without publishing"

    def run(self, context):
        _, rel, branch = _ws()
        _server("claims/release", {"branch": branch, "paths": [rel]})
        return "Released."


class MP_OT_ask(_Action):
    bl_idname = "multiparity.ask"
    bl_label = "Ask to release"
    bl_description = "Tell whoever has this file claimed that you are waiting for it"

    def run(self, context):
        _, rel, branch = _ws()
        _server("claims/nudge", {"branch": branch, "path": rel})
        return "Asked. They get a notification."


class MP_OT_checkpoint(_Action):
    bl_idname = "multiparity.checkpoint"
    bl_label = "Save checkpoint"
    bl_description = "Save the file, then keep a private save point you can rewind to"
    note: bpy.props.StringProperty(name="Note", default="")

    def invoke(self, context, event):
        return context.window_manager.invoke_props_dialog(self)

    def run(self, context):
        bpy.ops.wm.save_mainfile()
        ws, _, _ = _ws()
        call("POST", "/workspaces/%s/checkpoint" % ws, {"note": self.note or None})
        return "Checkpoint saved."


class MP_OT_publish(_Action):
    bl_idname = "multiparity.publish"
    bl_label = "Publish"
    bl_description = "Save the file and publish it to everyone as one change"
    message: bpy.props.StringProperty(name="Say what you changed", default="")

    def invoke(self, context, event):
        return context.window_manager.invoke_props_dialog(self, width=420)

    def run(self, context):
        if not self.message.strip():
            raise RuntimeError("Say what you changed, so the team knows what this change is.")
        bpy.ops.wm.save_mainfile()
        ws, rel, _ = _ws()
        r = call("POST", "/workspaces/%s/publish" % ws, {"message": self.message.strip(), "paths": [rel]}, timeout=3600)
        return "Published change %s." % r["change"]["number"]


class MP_OT_get_latest(_Action):
    bl_idname = "multiparity.get_latest"
    bl_label = "Get latest"
    bl_description = "Get everyone's newest files; reopens this file if a newer version came in"

    def run(self, context):
        ws, rel, _ = _ws()
        before = (_state["info"] or {}).get("change")
        r = call("POST", "/workspaces/%s/get-latest" % ws, {}, timeout=3600)
        refresh()
        after = (_state["info"] or {}).get("change")
        if after != before and not bpy.data.is_dirty:
            bpy.ops.wm.revert_mainfile()
            return "Got latest and reopened the newer version of this file."
        kept = r.get("kept_yours") or []
        if rel in kept:
            return "Got latest. Kept your edited copy of this file; someone published a newer one."
        return "Got latest: %s file(s)." % r.get("downloaded", 0)


class MP_OT_versions(_Action):
    bl_idname = "multiparity.versions"
    bl_label = "Older versions"
    bl_description = "List the published versions of this file"

    def run(self, context):
        ws, rel, _ = _ws()
        r = call("GET", "/workspaces/%s/versions?path=%s" % (ws, urllib.parse.quote(rel)))
        _state["versions"] = [v for v in r.get("versions", []) if v.get("op") != "delete"][:12]
        return None

    def execute(self, context):
        try:
            self.run(context)
        except RuntimeError as e:
            self.report({"ERROR"}, str(e))
        return {"FINISHED"}


class MP_OT_put_back(_Action):
    bl_idname = "multiparity.put_back"
    bl_label = "Put back"
    bl_description = "Open this version in place of the file (your unsaved and unpublished work goes into a checkpoint first). Publish it to make it the latest"
    change: bpy.props.IntProperty()

    def run(self, context):
        if bpy.data.is_dirty:
            bpy.ops.wm.save_mainfile()
        ws, rel, _ = _ws()
        call("POST", "/workspaces/%s/versions" % ws, {"path": rel, "change": self.change, "to": "folder"}, timeout=3600)
        bpy.ops.wm.revert_mainfile()
        return "Opened the version from change %d. Publish it to make it the latest." % self.change


class MP_PT_panel(bpy.types.Panel):
    bl_label = "MultiParity"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "MultiParity"

    def draw(self, context):
        col = self.layout.column(align=False)
        if not bpy.data.filepath:
            col.label(text="Save the file into a synced folder first.", icon="INFO")
            return
        if _state["error"]:
            box = col.box()
            for line in _wrap(_state["error"], 34):
                box.label(text=line)
            col.operator("multiparity.refresh", icon="FILE_REFRESH")
            return
        i = _state["info"]
        if i is None:
            col.operator("multiparity.refresh", icon="FILE_REFRESH")
            return
        if not i.get("synced"):
            col.label(text="This file is not in a synced folder.", icon="INFO")
            col.operator("multiparity.refresh", icon="FILE_REFRESH")
            return
        col.label(text="%s · %s" % (i["project"], i["branch"]), icon="FILE_FOLDER")
        state = {"new": "New, not published yet", "edited": "Edited, not published", "unchanged": "Up to date with change %s" % i.get("change"),
                 "deleted": "Deleted here", "ignored": "Ignored (never published)"}.get(i["state"], i["state"])
        col.label(text=state, icon="CHECKMARK" if i["state"] == "unchanged" else "GREASEPENCIL")
        if i.get("newer_on_server"):
            col.label(text="A newer version is on the server (change %s)" % i["server_change"], icon="ERROR")
        claim = i.get("claim")
        if claim and claim.get("mine"):
            col.label(text="Claimed by you", icon="LOCKED")
        elif claim:
            col.label(text="Claimed by %s" % (claim.get("user") or "someone"), icon="LOCKED")
        else:
            col.label(text="Not claimed", icon="UNLOCKED")
        col.separator()
        row = col.row(align=True)
        if claim and claim.get("mine"):
            row.operator("multiparity.release", icon="UNLOCKED")
        elif claim:
            row.operator("multiparity.ask", icon="USER")
        else:
            row.operator("multiparity.claim", icon="LOCKED")
        col.operator("multiparity.publish", icon="EXPORT")
        row = col.row(align=True)
        row.operator("multiparity.checkpoint", icon="BOOKMARKS")
        row.operator("multiparity.get_latest", icon="IMPORT")
        row = col.row(align=True)
        row.operator("multiparity.versions", icon="TIME")
        row.operator("multiparity.refresh", text="", icon="FILE_REFRESH")
        if _state["versions"]:
            box = col.box()
            for v in _state["versions"]:
                r = box.row(align=True)
                who = (v.get("author") or {}).get("name") or "someone"
                r.label(text="%s · %s: %s" % (v["change"], who.split(" ")[0], v.get("message", ""))[:40])
                if not v.get("current"):
                    op = r.operator("multiparity.put_back", text="", icon="LOOP_BACK")
                    op.change = v["change"]


def _wrap(text, width):
    words, lines, cur = text.split(), [], ""
    for w in words:
        if len(cur) + len(w) + 1 > width and cur:
            lines.append(cur)
            cur = w
        else:
            cur = (cur + " " + w).strip()
    return lines + ([cur] if cur else [])


@persistent
def _on_file(*_):
    # After the file opens or saves; let Blender finish first.
    bpy.app.timers.register(lambda: (refresh(), None)[1], first_interval=0.5)


classes = (MP_Prefs, MP_OT_refresh, MP_OT_claim, MP_OT_release, MP_OT_ask, MP_OT_checkpoint, MP_OT_publish,
           MP_OT_get_latest, MP_OT_versions, MP_OT_put_back, MP_PT_panel)


def register():
    for c in classes:
        bpy.utils.register_class(c)
    bpy.app.handlers.load_post.append(_on_file)
    bpy.app.handlers.save_post.append(_on_file)


def unregister():
    for h in (bpy.app.handlers.load_post, bpy.app.handlers.save_post):
        if _on_file in h:
            h.remove(_on_file)
    for c in reversed(classes):
        bpy.utils.unregister_class(c)
