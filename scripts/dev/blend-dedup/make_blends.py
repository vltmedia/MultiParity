# Builds a large .blend and saves it again after typical edits, to measure how much of the file
# changes between saves. Run by run.sh, one Blender session per stage:
#
#   blender -b --python make_blends.py -- <stage> <dir> [target_gb]
#
# Stages (each opens the previous file, as an artist would, and saves the next):
#   create   v1.blend  the scene: dense meshes plus packed textures, about target_gb
#            v2.blend  same session: one object moved
#   noop     n1.blend  v2 reopened in a new session and saved with no edit
#   move     v3.blend  v2 reopened, one object moved
#   sculpt   v4.blend  v3 reopened, every vertex of one dense mesh displaced
#   add      v5.blend  v4 reopened, one more dense mesh added
#   zip      v6.blend  v5 reopened, saved with Blender's Compress option
#   zipmove  v7.blend  v6 reopened, one object moved, saved compressed
import sys
import time

import bpy
import numpy as np

stage, out = sys.argv[sys.argv.index("--") + 1 :][:2]
target_gb = float(sys.argv[sys.argv.index("--") + 3]) if len(sys.argv) > sys.argv.index("--") + 3 else 4.0
rng = np.random.default_rng(7)


def dense_mesh(name, side, seed):
    """A side x side quad grid with noisy heights: about 65 bytes per vertex on disk."""
    r = np.random.default_rng(seed)
    xs, ys = np.meshgrid(np.arange(side, dtype=np.float32), np.arange(side, dtype=np.float32))
    co = np.stack([xs * 0.01, ys * 0.01, r.random((side, side), dtype=np.float32)], axis=-1).reshape(-1, 3)
    q = side - 1
    i = (np.arange(q)[:, None] * side + np.arange(q)[None, :]).reshape(-1).astype(np.int32)
    loops = np.stack([i, i + 1, i + side + 1, i + side], axis=-1).reshape(-1)
    me = bpy.data.meshes.new(name)
    me.vertices.add(len(co))
    me.loops.add(len(loops))
    me.polygons.add(q * q)
    me.vertices.foreach_set("co", co.reshape(-1))
    me.loops.foreach_set("vertex_index", loops)
    me.polygons.foreach_set("loop_start", np.arange(0, len(loops), 4, dtype=np.int32))
    try:
        me.polygons.foreach_set("loop_total", np.full(q * q, 4, dtype=np.int32))
    except Exception:
        pass  # read-only (derived from loop_start) in newer versions
    me.update(calc_edges=True)
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    return ob


def packed_texture(name, side, seed):
    """A noise texture packed into the file, as artists' scenes often carry their textures."""
    r = np.random.default_rng(seed)
    img = bpy.data.images.new(name, side, side, alpha=True)
    img.pixels.foreach_set(r.random(side * side * 4, dtype=np.float32))
    img.pack()
    return img


def save(name, compress=False):
    t = time.time()
    bpy.ops.wm.save_as_mainfile(filepath=f"{out}/{name}.blend", compress=compress)
    print(f"SAVED {name}.blend in {time.time() - t:.1f}s", flush=True)


def reopen(name):
    bpy.ops.wm.open_mainfile(filepath=f"{out}/{name}.blend")


def nudge():
    ob = bpy.data.objects["mesh_000"]
    ob.location.x += 1.0


print(f"STAGE {stage} Blender {bpy.app.version_string}", flush=True)
if stage == "create":
    bpy.ops.wm.read_factory_settings(use_empty=True)
    side = 1500  # 2.25 million vertices each
    meshes = max(1, round(target_gb * 0.65 * 1e9 / (side * side * 65)))
    textures = max(1, round(target_gb * 0.35 * 1e9 / 64e6))
    for n in range(meshes):
        dense_mesh(f"mesh_{n:03d}", side, 100 + n)
    for n in range(textures):
        packed_texture(f"tex_{n:03d}", 4096, 500 + n)
    print(f"built {meshes} meshes and {textures} packed textures", flush=True)
    save("v1")
    nudge()
    save("v2")
elif stage == "noop":
    reopen("v2")
    save("n1")
elif stage == "move":
    reopen("v2")
    nudge()
    save("v3")
elif stage == "sculpt":
    reopen("v3")
    me = bpy.data.meshes["mesh_001"]
    co = np.empty(len(me.vertices) * 3, dtype=np.float32)
    me.vertices.foreach_get("co", co)
    co[2::3] += rng.random(len(me.vertices), dtype=np.float32) * 0.1
    me.vertices.foreach_set("co", co)
    me.update()
    save("v4")
elif stage == "add":
    reopen("v4")
    dense_mesh("mesh_new", 1500, 999)
    save("v5")
elif stage == "zip":
    reopen("v5")
    save("v6", compress=True)
elif stage == "zipmove":
    reopen("v6")
    nudge()
    save("v7", compress=True)
else:
    raise SystemExit(f"unknown stage {stage}")
