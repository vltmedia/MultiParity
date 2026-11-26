#!/usr/bin/env bash
# Fill a server with demo data: two organizations, projects, people, files, a branch, tags and
# claims. Runs the `mp` CLI from the server image so nothing is installed on the host.
#
#   cd <deploy dir with .env> && bash scripts/dev/seed-demo.sh
#
# The admin account comes from .env. Demo people get the password in DEMO_PASSWORD (default:
# a random one, printed to demo-accounts.txt next to .env with owner-only permissions).
set -euo pipefail
cd "$(dirname "$0")/../.."
set -a; . ./.env; set +a
SERVER="http://127.0.0.1:${MP_PORT:-18500}"
DEMO_PASSWORD="${DEMO_PASSWORD:-$(head -c 24 /dev/urandom | base64 | tr -dc A-Za-z0-9 | head -c 16)}"
VOL=mp-demo

run() { docker run --rm -i --network host -v $VOL:/data -e MP_CONFIG_DIR="/data/cfg-$1" -e MP_DEVICE="$1-workstation" --user 0 multiparity:latest "${@:2}"; }
mp_as() { local who="$1"; shift; run "$who" mp "$@"; }
sh_as() { local who="$1"; shift; run "$who" sh -c "$*"; }

docker volume create $VOL >/dev/null
mp_as admin login "$SERVER" --email "$MP_ADMIN_EMAIL" --password "$MP_ADMIN_PASSWORD" --label studio >/dev/null
TOKEN=$(sh_as admin 'cat /data/cfg-admin/accounts.json' | sed -n 's/.*"token": "\([^"]*\)".*/\1/p' | head -1)
H=(-H "authorization: Bearer $TOKEN" -H 'content-type: application/json')
ORG1=$(curl -fs "$SERVER/api/orgs" "${H[@]}" | sed -n 's/.*"id":"\([^"]*\)","slug":"[^"]*","name":"'"${MP_DEFAULT_ORG}"'".*/\1/p' | head -1)
ORG2=$(curl -fs -X POST "$SERVER/api/orgs" "${H[@]}" -d '{"name":"Northlight Games"}' | sed -n 's/.*"id":"\([^"]*\)".*/\1/p' | head -1 || true)
for p in Emberfall Tidewrack; do curl -fs -X POST "$SERVER/api/orgs/$ORG1/projects" "${H[@]}" -d "{\"name\":\"$p\"}" >/dev/null || true; done
[ -n "$ORG2" ] && curl -fs -X POST "$SERVER/api/orgs/$ORG2/projects" "${H[@]}" -d '{"name":"Harbor DLC"}' >/dev/null || true

for who in mara:Mara teo:Teo dev:Dev sana:Sana; do
  u="${who%%:*}"; n="${who##*:}"
  curl -fs -X POST "$SERVER/api/admin/users" "${H[@]}" -d "{\"email\":\"$u@studio.example\",\"name\":\"$n\",\"password\":\"$DEMO_PASSWORD\",\"org\":\"$ORG1\",\"role\":\"member\"}" >/dev/null || true
  mp_as "$u" login "$SERVER" --email "$u@studio.example" --password "$DEMO_PASSWORD" >/dev/null
done

# Admin publishes the first pass of Emberfall.
sh_as admin "mp connect Emberfall /data/work/admin/Emberfall >/dev/null && cd /data/work/admin/Emberfall && \
  mkdir -p Characters/Hero Characters/EmberWolf Environments/Forge Audio Cinematics && \
  for f in Characters/Hero/hero_body.fbx:9000000 Characters/Hero/hero_body_diffuse.png:3000000 Characters/Hero/hero_cape_sim.abc:14000000 \
           Characters/EmberWolf/ember_wolf.fbx:6000000 Characters/EmberWolf/ember_wolf_normal.png:2500000 Environments/Forge/forge_door.fbx:1200000 \
           Environments/Forge/forge_door_rust.png:1800000 Environments/Forge/forge_interior.umap:4000000 Audio/campfire_loop.wav:900000 \
           Audio/wolf_howl_03.wav:300000 Cinematics/intro_cinematic.mov:20000000; do p=\${f%%:*}; n=\${f##*:}; head -c \$n /dev/urandom > \$p; done && \
  printf 'Emberfall art bible\n\n- Hero: ember-lit steel\n- Wolf: ash fur\n' > Characters/README.md && \
  mp publish -m 'First pass of the hero, wolf and forge' >/dev/null && \
  mp tag new Approved --group status --color '#00806a' >/dev/null && mp tag new 'Needs review' --group status --color '#ff6a13' >/dev/null && \
  mp tag new 'Hero asset' --color '#7a3cff' >/dev/null && mp tag add 'Hero asset' Characters/Hero >/dev/null && mp tag add Approved Environments/Forge/forge_door.fbx >/dev/null"

# Others connect, edit, publish and claim.
sh_as mara "mp connect Emberfall /data/work/mara/Emberfall >/dev/null && cd /data/work/mara/Emberfall && head -c 200000 /dev/urandom >> Characters/Hero/hero_cape_sim.abc && mp publish -m 'Cape sim, longer settle so it stops popping' >/dev/null && mp claim Characters/Hero/hero_cape_sim.abc >/dev/null"
sh_as dev "mp connect Emberfall /data/work/dev/Emberfall >/dev/null && cd /data/work/dev/Emberfall && head -c 300000 /dev/urandom >> Environments/Forge/forge_interior.umap && mp publish -m 'Forge interior lighting rebuilt after door change' >/dev/null && mp claim Environments/Forge/forge_interior.umap >/dev/null && head -c 400000 /dev/urandom >> Cinematics/intro_cinematic.mov && mp publish -m 'Intro cinematic, colour pass and new end card' >/dev/null"
sh_as teo "mp connect Emberfall /data/work/teo/Emberfall >/dev/null && cd /data/work/teo/Emberfall && mp claim Environments/Forge/forge_door_rust.png >/dev/null && mp branch new 'Wolf rework' >/dev/null && mp branch switch 'Wolf rework' >/dev/null && head -c 100000 /dev/urandom >> Characters/EmberWolf/ember_wolf.fbx && mp publish -m 'Wolf topology pass' >/dev/null"
sh_as sana "mp connect Emberfall /data/work/sana/Emberfall >/dev/null && cd /data/work/sana/Emberfall && head -c 50000 /dev/urandom >> Characters/Hero/hero_body.fbx && mp publish -m 'Hero body retopo, first pass' >/dev/null"

# The admin's folder is behind and has unpublished work, for the "Your work" panel.
sh_as admin "cd /data/work/admin/Emberfall && head -c 20000 /dev/urandom >> Characters/Hero/hero_body_diffuse.png && head -c 700000 /dev/urandom > Characters/Hero/hero_body_normal.png && mp checkpoint -m 'before normal map bake' >/dev/null && mp claim Characters/Hero/hero_body_diffuse.png >/dev/null"
sh_as admin "mp connect Tidewrack /data/work/admin/Tidewrack >/dev/null && cd /data/work/admin/Tidewrack && mkdir -p Levels Props && head -c 2000000 /dev/urandom > Levels/harbor.umap && head -c 800000 /dev/urandom > Props/crate.fbx && mp publish -m 'Harbor blockout' >/dev/null"

umask 077; printf 'Demo people (password for all): %s\n%s\n' "$DEMO_PASSWORD" "mara@studio.example teo@studio.example dev@studio.example sana@studio.example" > demo-accounts.txt
echo "Seeded. Demo accounts are listed in $(pwd)/demo-accounts.txt"
