#!/usr/bin/env bash
# End-to-end test: a real server on Postgres and local storage, two people (Alice, an admin, and
# Bob, a member) using the `mp` CLI in separate folders and config directories.
#
# Needs: built binaries in target/debug, a Postgres reachable at $E2E_DB_ADMIN_URL (a database
# called mp_e2e is dropped and recreated), jq and curl.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${E2E_BIN:-$ROOT/target/debug}"
DB_ADMIN="${E2E_DB_ADMIN_URL:-postgres://mp:mp@127.0.0.1:18501/mp}"
PORT="${E2E_PORT:-18502}"
WORK="$(mktemp -d /tmp/mp-e2e.XXXXXX)"
SERVER="http://127.0.0.1:$PORT"
PASS_A="alice-password-123"
PASS_B="bob-password-1234"
STEP=0

step() { STEP=$((STEP+1)); echo; echo "== $STEP. $*"; }
fail() { echo "FAIL: $*" >&2; echo "--- server log"; tail -40 "$WORK/server.log" >&2 || true; exit 1; }
check() { local what="$1"; shift; if "$@"; then echo "   ok: $what"; else fail "$what"; fi; }
same() { [ "$(sha256sum < "$1")" = "$(sha256sum < "$2")" ]; }
alice() { MP_CONFIG_DIR="$WORK/cfg-alice" MP_DEVICE=alice-pc "$BIN/mp" "$@"; }
bob() { MP_CONFIG_DIR="$WORK/cfg-bob" MP_DEVICE=bob-pc "$BIN/mp" "$@"; }
cleanup() { [ -n "${SPID:-}" ] && kill "$SPID" 2>/dev/null || true; [ -n "${DPID:-}" ] && kill "$DPID" 2>/dev/null || true; }
trap cleanup EXIT

step "Fresh database and server"
psql_url="${DB_ADMIN%/*}"
curl -s >/dev/null || true
python3 - <<PY 2>/dev/null || true
PY
# Recreate the database through the postgres container's psql if available, else via the server's migrations on a new name.
DBNAME="mp_e2e_$(date +%s)"
if command -v psql >/dev/null; then
  psql "$DB_ADMIN" -qc "CREATE DATABASE $DBNAME" >/dev/null
else
  echo "psql not found; install postgresql-client" >&2; exit 2
fi
export DATABASE_URL="${psql_url}/$DBNAME"
MP_DESKTOP_DIR="$WORK/desktop" MP_BACKUP_TARGET="local:$WORK/backups" MP_STORAGE_LOCATIONS="second=local:$WORK/blobs2" MP_LISTEN="127.0.0.1:$PORT" MP_PUBLIC_URL="$SERVER" MP_STORAGE="${E2E_STORAGE:-local}" MP_STORAGE_PATH="${E2E_STORAGE_PATH:-$WORK/blobs}" \
  MP_ADMIN_EMAIL=alice@example.com MP_ADMIN_PASSWORD="$PASS_A" MP_DEFAULT_ORG="Cinder Works" \
  RUST_LOG=info,tower_http=warn "$BIN/mp-server" > "$WORK/server.log" 2>&1 &
SPID=$!
for i in $(seq 1 50); do curl -fs "$SERVER/api/health" >/dev/null && break; sleep 0.2; done
check "server answers /api/health" curl -fs "$SERVER/api/health"

step "Alice signs in, creates a project and invites Bob"
alice login "$SERVER" --email alice@example.com --password "$PASS_A" --label work >/dev/null
check "alice has one account" test "$(alice accounts --json | jq length)" = 1
alice admin new-project cinder-works Emberfall >/dev/null
check "project listed" test "$(alice projects --json | jq -r '.[0].project.name')" = Emberfall
INVITE_URL=$(alice admin invite cinder-works bob@example.com --role member --json | jq -r '.[0].url')
INVITE_TOKEN="${INVITE_URL##*/}"
curl -fs -X POST "$SERVER/api/invites/$INVITE_TOKEN/accept" -H 'content-type: application/json' -d "{\"name\":\"Bob\",\"password\":\"$PASS_B\"}" >/dev/null
bob login "$SERVER" --email bob@example.com --password "$PASS_B" >/dev/null
check "bob sees the project" test "$(bob projects --json | jq -r '.[0].project.name')" = Emberfall
ORG_ID=$(alice projects --json | jq -r '.[0].project.org.id')
ATOK=$(jq -r '.accounts[0].token' "$WORK/cfg-alice/accounts.json")
check "used invites no longer show a link" test "$(curl -fs "$SERVER/api/admin/orgs/$ORG_ID/invites" -H "authorization: Bearer $ATOK" | jq -r '.[0].url')" = null
OPEN_URL=$(alice admin invite cinder-works --role guest --json | jq -r '.[0].url')
check "open invites can be copied again from the list" test "$(curl -fs "$SERVER/api/admin/orgs/$ORG_ID/invites" -H "authorization: Bearer $ATOK" | jq -r --arg u "$OPEN_URL" '[.[] | select(.url==$u)] | length')" = 1
check "invite cannot be reused" bash -c "! curl -fs -X POST '$SERVER/api/invites/$INVITE_TOKEN/accept' -H 'content-type: application/json' -d '{\"name\":\"X\",\"password\":\"xxxxxxxxxxxx\"}' >/dev/null"

step "Alice connects an empty folder and publishes files"
A="$WORK/alice/Emberfall"; B="$WORK/bob/Emberfall"
alice connect Emberfall "$A" >/dev/null
mkdir -p "$A/Art/Hero" "$A/Audio" "$A/Docs"
head -c 12000000 /dev/urandom > "$A/Art/Hero/hero_body.fbx"
head -c 300000 /dev/urandom > "$A/Art/Hero/hero_diffuse.png"
head -c 50000 /dev/urandom > "$A/Audio/wolf_howl.wav"
printf 'line one\nline two\nline three\nline four\nline five\n' > "$A/Docs/notes.txt"
touch "$A/Art/Hero/.DS_Store"
check "status shows 4 new and ignores .DS_Store" test "$(alice -C "$A" status --json | jq '.status.new | length')" = 4
check "the ignored file is listed" test "$(alice -C "$A" status --json | jq -r '.status.ignored[0]')" = "Art/Hero/.DS_Store"
alice -C "$A" publish -m "First pass of the hero" >/dev/null
check "clean after publish" test "$(alice -C "$A" status --json | jq '.status.new | length')" = 0
check "change 1 in the log" test "$(alice -C "$A" log --json | jq '.[0].number')" = 1

step "Bob connects and gets the same files"
bob connect Emberfall "$B" >/dev/null
check "hero_body.fbx identical" same "$A/Art/Hero/hero_body.fbx" "$B/Art/Hero/hero_body.fbx"
check "notes.txt identical" same "$A/Docs/notes.txt" "$B/Docs/notes.txt"
check "bob is clean" test "$(bob -C "$B" status --json | jq '[.status.new, .status.edited, .status.deleted] | map(length) | add')" = 0

step "Claims stop Alice from publishing over Bob"
bob -C "$B" claim Art/Hero/hero_body.fbx >/dev/null
check "alice sees bob's claim" test "$(alice -C "$A" claims --json | jq -r '.[0].user.name')" = Bob
printf 'alice edit' >> "$A/Art/Hero/hero_body.fbx"
if alice -C "$A" publish -m "Alice edits the body" > "$WORK/out.json" 2>&1; then fail "publish over a claim should fail"; fi
check "the refusal names the claim" grep -q "claimed by" "$WORK/out.json"
alice -C "$A" discard Art/Hero/hero_body.fbx >/dev/null
check "discard restored alice's copy" same "$A/Art/Hero/hero_body.fbx" "$B/Art/Hero/hero_body.fbx"

step "Bob publishes an edit (only changed chunks are sent); Alice gets latest"
python3 - "$B/Art/Hero/hero_body.fbx" <<'PY' 2>/dev/null || dd if=/dev/urandom of="$B/Art/Hero/hero_body.fbx" bs=1 count=64 seek=6000000 conv=notrunc 2>/dev/null
import sys
p = sys.argv[1]
with open(p, 'r+b') as f:
    f.seek(6_000_000); f.write(b'\xff' * 64)
PY
R=$(bob -C "$B" publish -m "Tweak the body" --json)
check "bob's claim was released by publishing" test "$(bob -C "$B" claims --json | jq length)" = 0
SENT=$(echo "$R" | jq '.uploaded_bytes')
check "less than 5 MB sent for a 12 MB file ($SENT bytes)" test "$SENT" -lt 5000000
alice -C "$A" get-latest >/dev/null
check "alice has bob's edit" same "$A/Art/Hero/hero_body.fbx" "$B/Art/Hero/hero_body.fbx"

step "Stale edits are refused: Alice must get latest first"
printf 'alice wolf' >> "$A/Audio/wolf_howl.wav"
printf 'bob wolf' >> "$B/Audio/wolf_howl.wav"
bob -C "$B" publish -m "Bob's howl" >/dev/null
if alice -C "$A" publish -m "Alice's howl" > "$WORK/out.txt" 2>&1; then fail "a stale publish should fail"; fi
check "the refusal says it changed on the server" grep -q "changed on the server" "$WORK/out.txt"
alice -C "$A" get-latest > "$WORK/out.txt"
check "get latest kept alice's edited copy" grep -q "kept your version of Audio/wolf_howl.wav" "$WORK/out.txt"
alice -C "$A" discard Audio/wolf_howl.wav >/dev/null
check "discard took the server's newer version" same "$A/Audio/wolf_howl.wav" "$B/Audio/wolf_howl.wav"
printf 'alice again' >> "$A/Audio/wolf_howl.wav"
alice -C "$A" publish -m "Alice's howl over Bob's" --keep-mine Audio/wolf_howl.wav >/dev/null
bob -C "$B" get-latest >/dev/null
check "keep-mine put alice's version on the server" same "$A/Audio/wolf_howl.wav" "$B/Audio/wolf_howl.wav"

step "Checkpoints and rewind"
printf 'draft A\n' > "$A/Docs/plan.txt"
CP=$(alice -C "$A" checkpoint -m "before rewrite" --json | jq -r .checkpoint.id)
printf 'draft B\n' > "$A/Docs/plan.txt"
alice -C "$A" rewind "$CP" >/dev/null
check "plan.txt is back to draft A" grep -q "draft A" "$A/Docs/plan.txt"
check "two checkpoints exist (named + the safety one)" test "$(alice -C "$A" checkpoints --json | jq length)" -ge 2

step "Disk space: no second copy of synced or published files"
parts() { find "$1/.mp/cache" -type f 2>/dev/null | wc -l; }
check "bob downloaded and published but keeps no copies ($(parts "$B") parts)" test "$(parts "$B")" = 0
SPACE=$(MP_CACHE_GRACE_SECS=0 alice -C "$A" space --json)
echo "   alice: $(echo "$SPACE" | jq -c '{kept_bytes, freed_bytes, checked_server}')"
check "checkpoints of a 12 MB edit keep only the parts the server lacks (at most 2 parts)" test "$(echo "$SPACE" | jq .kept_bytes)" -lt 9000000
check "parts the server has were never written here (nothing to free)" test "$(echo "$SPACE" | jq .freed_bytes)" = 0
check "the server was asked" test "$(echo "$SPACE" | jq .checked_server)" = true
DCP=$(alice -C "$A" checkpoints --json | jq -r '[.[] | select(.note=="Before discarding" and (.files | map(.path) | index("Art/Hero/hero_body.fbx")))] | last | .id')
alice -C "$A" rewind "$DCP" Art/Hero/hero_body.fbx >/dev/null
check "rewind rebuilt the old edit from server parts plus local ones" bash -c "[ \"\$(tail -c 10 '$A/Art/Hero/hero_body.fbx')\" = 'alice edit' ] && [ \$(stat -c %s '$A/Art/Hero/hero_body.fbx') = 12000010 ]"
alice -C "$A" discard Art/Hero/hero_body.fbx >/dev/null
check "and discarding brought back the latest" same "$A/Art/Hero/hero_body.fbx" "$B/Art/Hero/hero_body.fbx"
SPACE=$(MP_CACHE_GRACE_SECS=0 alice -C "$A" space --limit 0 --json)
check "a zero limit never removes checkpoints people made" test "$(alice -C "$A" checkpoints --json | jq --arg id "$CP" '[.[] | select(.id==$id)] | length')" = 1
alice -C "$A" space --limit auto >/dev/null

step "Drafts on the server"
alice -C "$A" draft save -m "Plan in progress" >/dev/null
DID=$(alice -C "$A" draft list --json | jq -r '.[0].id')
check "bob cannot see alice's draft" test "$(bob -C "$B" draft list --json | jq length)" = 0
alice -C "$A" discard Docs/plan.txt >/dev/null
check "discard moved the new file aside" test ! -e "$A/Docs/plan.txt"
alice -C "$A" draft restore "$DID" >/dev/null
check "draft restored plan.txt" grep -q "draft A" "$A/Docs/plan.txt"
alice -C "$A" publish -m "Add the plan" >/dev/null
bob -C "$B" get-latest >/dev/null

step "Branches: text edits on both sides merge automatically"
alice -C "$A" branch new "Wolf rework" >/dev/null
alice -C "$A" branch switch "Wolf rework" >/dev/null
sed -i 's/line one/LINE ONE (alice)/' "$A/Docs/notes.txt"
alice -C "$A" publish -m "Notes on the branch" >/dev/null
sed -i 's/line five/LINE FIVE (bob)/' "$B/Docs/notes.txt"
bob -C "$B" publish -m "Notes on Main" >/dev/null
check "branch is behind Main by 1" test "$(alice -C "$A" branch list --json | jq '.[] | select(.name=="Wolf rework") | .behind')" = 1
if alice -C "$A" merge > "$WORK/out.txt" 2>&1; then fail "merging while behind should fail"; fi
check "merge says to update first" grep -q "Update from it first" "$WORK/out.txt"
alice -C "$A" update > "$WORK/out.txt"
check "update merged notes.txt automatically" grep -q "1 merged automatically" "$WORK/out.txt"
alice -C "$A" get-latest >/dev/null
check "both edits in alice's notes" bash -c "grep -q 'LINE ONE (alice)' '$A/Docs/notes.txt' && grep -q 'LINE FIVE (bob)' '$A/Docs/notes.txt'"
alice -C "$A" merge -m "Wolf rework into Main" >/dev/null
bob -C "$B" get-latest >/dev/null
check "bob's Main has both edits" bash -c "grep -q 'LINE ONE (alice)' '$B/Docs/notes.txt' && grep -q 'LINE FIVE (bob)' '$B/Docs/notes.txt'"
alice -C "$A" branch switch Main >/dev/null
check "alice is back on Main and clean" test "$(alice -C "$A" status --json | jq -r .branch)" = Main

step "Branches: binary clash on a branch with its own claims needs a choice"
alice -C "$A" branch new "Demo" --own-claims >/dev/null
alice -C "$A" branch switch Demo >/dev/null
printf 'demo' >> "$A/Art/Hero/hero_diffuse.png"
alice -C "$A" publish -m "Demo texture" >/dev/null
printf 'main' >> "$B/Art/Hero/hero_diffuse.png"
bob -C "$B" publish -m "Main texture" >/dev/null
alice -C "$A" update > "$WORK/out.txt"
check "update reports the binary clash" grep -q "Art/Hero/hero_diffuse.png (binary)" "$WORK/out.txt"
alice -C "$A" update --keep "Art/Hero/hero_diffuse.png=mine" >/dev/null
alice -C "$A" merge >/dev/null
bob -C "$B" get-latest >/dev/null
check "main now has the demo texture" grep -q "demo" "$B/Art/Hero/hero_diffuse.png"
alice -C "$A" branch switch Main >/dev/null

step "Connecting a folder that already has files keeps them"
C="$WORK/carol/Emberfall"
mkdir -p "$(dirname "$C")"; cp -r "$B" "$C"; rm -rf "$C/.mp"
printf 'local only' > "$C/Docs/local_only.txt"
printf 'changed here' >> "$C/Docs/plan.txt"
rm "$C/Audio/wolf_howl.wav"
REPORT=$(bob connect Emberfall "$C" --dry-run --json | jq .report)
echo "   report: $(echo "$REPORT" | jq -c '{different, local_only, to_download}')"
check "dry run: 1 different, 1 local only, 1 to download" test "$(echo "$REPORT" | jq -c '[(.different|length), .local_only, .to_download]')" = "[1,1,1]"
check "dry run changed nothing" test ! -e "$C/.mp"
bob connect Emberfall "$C" >/dev/null
check "local-only file kept" test -e "$C/Docs/local_only.txt"
check "edited file kept" grep -q "changed here" "$C/Docs/plan.txt"
check "missing file downloaded" test -e "$C/Audio/wolf_howl.wav"
check "status shows the kept files" test "$(bob -C "$C" status --json | jq -c '[(.status.new|length), (.status.edited|length)]')" = "[1,1]"

step "Overlapping folders are refused"
if bob connect Emberfall "$C/Art" > "$WORK/out.txt" 2>&1; then fail "nested connect should fail"; fi
check "refusal mentions the existing folder" grep -q "inside a connected folder" "$WORK/out.txt"

step "Selective sync removes only unchanged files"
printf 'unpublished' >> "$B/Docs/notes.txt"
bob -C "$B" folders --only Art >/dev/null
check "Audio removed (unchanged)" test ! -e "$B/Audio/wolf_howl.wav"
check "edited Docs/notes.txt kept" test -e "$B/Docs/notes.txt"
check "Art still there" test -e "$B/Art/Hero/hero_body.fbx"
bob -C "$B" folders --only "" >/dev/null
check "Audio back after including everything" test -e "$B/Audio/wolf_howl.wav"
bob -C "$B" discard Docs/notes.txt >/dev/null

step "Ignore suggestions and rules"
head -c 1000 /dev/urandom > "$A/Art/Hero/hero_body_autosave_0142.blend1"
head -c 1000 /dev/urandom > "$A/Art/Hero/hero_body_autosave_0143.blend1"
SUG=$(alice -C "$A" ignore --suggest Art/Hero/hero_body_autosave_0142.blend1 --json)
check "suggests the numbered pattern matching 2 files" test "$(echo "$SUG" | jq '.[] | select(.pattern=="hero_body_autosave_*.blend1") | .matches')" = 2
alice -C "$A" ignore "*autosave*" >/dev/null
check "autosaves no longer show as new" test "$(alice -C "$A" status --json | jq '[.status.new[].path | select(test("autosave"))] | length')" = 0
check "check names the rule" test "$(alice -C "$A" ignore --check Art/Hero/hero_body_autosave_0142.blend1 --json | jq -r .ignored)" = true
alice -C "$A" publish -m "Ignore autosaves" >/dev/null
bob -C "$B" get-latest >/dev/null
check ".mpignore reached bob" grep -q autosave "$B/.mpignore"

step "Tags"
alice -C "$A" tag new Approved --group status >/dev/null
alice -C "$A" tag new "Needs review" --group status >/dev/null
alice -C "$A" tag add "Needs review" Art >/dev/null
alice -C "$A" tag add Approved Art/Hero/hero_body.fbx >/dev/null
PID=$(alice projects --json | jq -r '.[0].project.id')
TOKEN=$(jq -r '.accounts[0].token' "$WORK/cfg-alice/accounts.json")
MAIN=$(curl -fs "$SERVER/api/projects/$PID/branches" -H "authorization: Bearer $TOKEN" | jq -r '.[] | select(.name=="Main") | .id')
APPROVED=$(curl -fs "$SERVER/api/projects/$PID/tags" -H "authorization: Bearer $TOKEN" | jq -r '.[] | select(.name=="Approved") | .id')
check "filter by Approved finds the body" test "$(curl -fs "$SERVER/api/projects/$PID/branches/$MAIN/files?tag=$APPROVED" -H "authorization: Bearer $TOKEN" | jq -r '.[0].path')" = "Art/Hero/hero_body.fbx"
check "folder tag is inherited" test "$(curl -fs "$SERVER/api/projects/$PID/branches/$MAIN/files?prefix=Art/Hero" -H "authorization: Bearer $TOKEN" | jq '[.[] | select(.tags | map(.name) | index("Needs review"))] | length')" -ge 2

step "Admin: suspend and reactivate Bob"
alice admin suspend bob@example.com --reason "testing" >/dev/null
if bob -C "$B" claims > "$WORK/out.txt" 2>&1; then fail "a suspended account should be refused"; fi
check "bob is told he is suspended" grep -q "suspended" "$WORK/out.txt"
alice admin reactivate bob@example.com >/dev/null
bob login "$SERVER" --email bob@example.com --password "$PASS_B" >/dev/null
check "bob works again" bob -C "$B" claims --json

step "Sign-in protection: wrong passwords make an account wait"
login_code() { curl -s -o /dev/null -w '%{http_code}' -X POST "$SERVER/api/auth/login" -H 'content-type: application/json' -d "{\"email\":\"$1\",\"password\":\"$2\"}"; }
for i in $(seq 1 8); do login_code bob@example.com "wrong-password-$i" >/dev/null; done
check "after 8 wrong passwords even the right one waits (429)" test "$(login_code bob@example.com "$PASS_B")" = 429
check "the refusal says how long" bash -c "curl -s -X POST '$SERVER/api/auth/login' -H 'content-type: application/json' -d '{\"email\":\"bob@example.com\",\"password\":\"x\"}' | jq -r .error.message | grep -q 'Try again in'"
check "other accounts are not affected" test "$(login_code alice@example.com "$PASS_A")" = 200
check "an unknown email is refused like a wrong password (401)" test "$(login_code nobody@example.com whatever)" = 401
check "the lock is in the audit log" test "$(alice admin audit --json | jq '[.[] | select(.kind=="auth.locked")] | length')" -ge 1
BOB_UID=$(curl -fs "$SERVER/api/admin/users?q=bob" -H "authorization: Bearer $TOKEN" | jq -r '.[0].id')
curl -fs -X POST "$SERVER/api/admin/users/$BOB_UID/password" -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' -d "{\"password\":\"$PASS_B\"}" >/dev/null
check "an admin setting a new password ends the wait" test "$(login_code bob@example.com "$PASS_B")" = 200
bob login "$SERVER" --email bob@example.com --password "$PASS_B" >/dev/null

step "Admin: stale claims, storage check, garbage collection preview, audit"
bob -C "$B" claim Audio/wolf_howl.wav >/dev/null
check "stale claims (0h) lists bob's claim" test "$(alice admin stale-claims --hours 0 --json | jq length)" -ge 1
check "storage check passes" test "$(alice admin check --json | jq .ok)" = true
check "gc preview answers" curl -fs "$SERVER/api/admin/storage/gc" -H "authorization: Bearer $TOKEN"
check "audit log has events" test "$(alice admin audit --json | jq length)" -ge 10
BOB_ID=$(curl -fs "$SERVER/api/admin/users?q=bob" -H "authorization: Bearer $TOKEN" | jq -r '.[0].id')
check "person page lists claims" test "$(curl -fs "$SERVER/api/admin/users/$BOB_ID" -H "authorization: Bearer $TOKEN" | jq '.claims | type')" = '"array"'
check "bob cannot open admin" bash -c "! MP_CONFIG_DIR='$WORK/cfg-bob' '$BIN/mp' admin overview >/dev/null 2>&1"

step "Download endpoint streams the current file"
curl -fs "$SERVER/api/projects/$PID/download?branch=$MAIN&path=Art/Hero/hero_body.fbx" -H "authorization: Bearer $TOKEN" -o "$WORK/dl.bin"
check "downloaded body matches" same "$WORK/dl.bin" "$A/Art/Hero/hero_body.fbx"

step "Older versions: list, download, put back, save a copy, make latest"
printf 'brief, first version\n' > "$A/Docs/brief.txt"
V1=$(alice -C "$A" publish -m "Brief v1" Docs/brief.txt --json | jq .change.number)
cp "$A/Docs/brief.txt" "$WORK/brief-v1.txt"
printf 'brief, second version\n' > "$A/Docs/brief.txt"
V2=$(alice -C "$A" publish -m "Brief v2" Docs/brief.txt --json | jq .change.number)
cp "$A/Docs/brief.txt" "$WORK/brief-v2.txt"
VERS=$(alice -C "$A" versions Docs/brief.txt --json)
check "two versions, newest first, newest marked latest" test "$(echo "$VERS" | jq -c '[.[].change, .[0].current, .[1].current]')" = "[$V2,$V1,true,false]"
curl -fs "$SERVER/api/projects/$PID/download?branch=$MAIN&path=Docs/brief.txt&change=$V1" -H "authorization: Bearer $TOKEN" -o "$WORK/dl-v1.txt"
check "downloading change $V1 gives the first version" same "$WORK/dl-v1.txt" "$WORK/brief-v1.txt"
alice -C "$A" versions Docs/brief.txt --copy "$V1" >/dev/null
check "a copy is saved beside the file" same "$A/Docs/brief (change $V1).txt" "$WORK/brief-v1.txt"
check "the file itself is untouched" same "$A/Docs/brief.txt" "$WORK/brief-v2.txt"
rm "$A/Docs/brief (change $V1).txt"
printf 'unpublished thoughts\n' > "$A/Docs/brief.txt"
alice -C "$A" versions Docs/brief.txt --get "$V1" > "$WORK/out.txt"
check "putting back v1 writes it into the folder" same "$A/Docs/brief.txt" "$WORK/brief-v1.txt"
check "it shows as edited" test "$(alice -C "$A" status --json | jq -r '.status.edited[].path' | grep -c '^Docs/brief.txt$')" = 1
UNDO=$(grep -o 'mp rewind [^ ]*' "$WORK/out.txt" | awk '{print $3}')
check "the unpublished edit went into a checkpoint" test -n "$UNDO"
alice -C "$A" rewind "$UNDO" Docs/brief.txt >/dev/null
check "rewinding gets the unpublished edit back" grep -q "unpublished thoughts" "$A/Docs/brief.txt"
alice -C "$A" discard Docs/brief.txt >/dev/null
bob -C "$B" claim Docs/brief.txt >/dev/null
if alice -C "$A" versions Docs/brief.txt --make-latest "$V1" > "$WORK/out.txt" 2>&1; then fail "make-latest over someone else's claim should fail"; fi
check "the refusal names the claim" grep -q "claimed by Bob" "$WORK/out.txt"
bob -C "$B" release Docs/brief.txt >/dev/null
if alice -C "$A" versions Docs/brief.txt --make-latest "$V2" > "$WORK/out.txt" 2>&1; then fail "the latest version cannot be made latest again"; fi
V3=$(alice -C "$A" versions Docs/brief.txt --make-latest "$V1" --json | jq .change.number)
check "making v1 the latest is a new change ($V3)" test "$V3" -gt "$V2"
check "alice's folder got it" same "$A/Docs/brief.txt" "$WORK/brief-v1.txt"
bob -C "$B" get-latest >/dev/null
check "bob gets it too" same "$B/Docs/brief.txt" "$WORK/brief-v1.txt"
check "the history keeps all three" test "$(alice -C "$A" versions Docs/brief.txt --json | jq length)" = 3

step "The server hands out the desktop app and its updates"
mkdir -p "$WORK/desktop"
head -c 200000 /dev/urandom > "$WORK/desktop/MultiParity_0.2.0_x64-setup.exe"
cat > "$WORK/desktop/latest.json" <<JSON
{"version":"0.2.0","notes":"Test release","pub_date":"2026-11-26T00:00:00Z",
 "platforms":{"windows-x86_64":{"url":"MultiParity_0.2.0_x64-setup.exe","signature":"c2lnbmF0dXJl"}},
 "downloads":{"windows-x86_64":"MultiParity_0.2.0_x64-setup.exe"}}
JSON
check "anyone can see what is published" test "$(curl -fs "$SERVER/api/desktop" | jq -c '[.version, .downloads[0].platform, .downloads[0].size]')" = '["0.2.0","windows-x86_64",200000]'
U=$(curl -fs "$SERVER/api/desktop/update/windows/x86_64/0.1.0")
check "an older app is offered 0.2.0 with its signature" test "$(echo "$U" | jq -c '[.version, .signature]')" = '["0.2.0","c2lnbmF0dXJl"]'
check "the update's address is on this server" test "$(echo "$U" | jq -r .url)" = "$SERVER/api/desktop/files/MultiParity_0.2.0_x64-setup.exe"
check "an up-to-date app is told there is nothing (204)" test "$(curl -s -o /dev/null -w '%{http_code}' "$SERVER/api/desktop/update/windows/x86_64/0.2.0")" = 204
check "a platform without a build is told there is nothing" test "$(curl -s -o /dev/null -w '%{http_code}' "$SERVER/api/desktop/update/darwin/aarch64/0.1.0")" = 204
curl -fs "$SERVER/api/desktop/files/MultiParity_0.2.0_x64-setup.exe" -o "$WORK/dl-setup.exe"
check "the installer downloads intact" same "$WORK/dl-setup.exe" "$WORK/desktop/MultiParity_0.2.0_x64-setup.exe"
check "nothing outside the folder is served" test "$(curl -s -o /dev/null -w '%{http_code}' "$SERVER/api/desktop/files/..%2Flatest.json")" = 404

step "Checkpoints leave out render output and caches"
mkdir -p "$A/renders" "$A/Art/Textures"
head -c 2000000 /dev/urandom > "$A/renders/shot_0001.exr"
head -c 1000 /dev/urandom > "$A/Art/Textures/rock.EXR"
printf 'todo\n' > "$A/Docs/todo.txt"
OUT=$(alice -C "$A" checkpoint -m "with renders about" --json)
check "the render and the EXR are left out" test "$(echo "$OUT" | jq -c '.left_out | sort')" = '["Art/Textures/rock.EXR","renders/shot_0001.exr"]'
check "the text file is in the checkpoint" test "$(echo "$OUT" | jq -c '[.checkpoint.files[].path]')" = '["Docs/todo.txt"]'
check "check names the built-in rule" test "$(alice -C "$A" no-checkpoint --check renders/shot_0001.exr --json | jq -r .left_out)" = true
printf '!Art/Textures/*.exr\n' >> "$A/.mpnocheckpoint"
check "a project exception brings textures back" test "$(alice -C "$A" no-checkpoint --check Art/Textures/rock.EXR --json | jq -r .left_out)" = false
alice -C "$A" no-checkpoint "*.bake" >/dev/null
head -c 1000 /dev/urandom > "$A/Art/lightmap.bake"
check "a rule added by hand applies" test "$(alice -C "$A" no-checkpoint --check Art/lightmap.bake --json | jq -r .left_out)" = true
check "--all includes everything" test "$(alice -C "$A" checkpoint -m "everything" --all --json | jq '.left_out | length')" = 0
alice -C "$A" publish -m "Renders, a texture and the no-checkpoint list" >/dev/null
cp "$A/renders/shot_0001.exr" "$WORK/shot.published"
printf 'rerendered' >> "$A/renders/shot_0001.exr"
cp "$A/renders/shot_0001.exr" "$WORK/shot.edited"
alice -C "$A" discard renders/shot_0001.exr >/dev/null
check "discard brought back the published render" same "$A/renders/shot_0001.exr" "$WORK/shot.published"
SAVED=$(find "$A/.mp/safety" -name shot_0001.exr | sort | tail -1)
check "the edited render was moved aside, not copied into a checkpoint" same "$SAVED" "$WORK/shot.edited"

step "A checkpoint that will not fit is not written"
MP_CACHE_GRACE_SECS=0 alice -C "$A" space --limit 1 >/dev/null
BEFORE=$(parts "$A")
head -c 6000000 /dev/urandom > "$A/Docs/big.bin"
OUT=$(alice -C "$A" checkpoint --auto --json)
echo "   $(echo "$OUT" | jq -c .too_big)"
check "the automatic checkpoint was skipped as too big" test "$(echo "$OUT" | jq '.too_big.needed_bytes')" -gt 5000000
check "and nothing was written to the cache" test "$(parts "$A")" = "$BEFORE"
check "a person's own checkpoint may still use the disk" test "$(alice -C "$A" checkpoint -m "big, on purpose" --json | jq -r '.checkpoint.files | length')" -ge 1
alice -C "$A" space --limit auto >/dev/null
check "the automatic limit is a share of the free disk" test "$(alice -C "$A" space --json | jq .limit_is_automatic)" = true

step "Checkpoints kept on the server"
AUTH=(-H "authorization: Bearer $TOKEN" -H 'content-type: application/json')
BTOK=$(jq -r '.accounts[0].token' "$WORK/cfg-bob/accounts.json")
check "the project starts on automatic (local for a small project)" test "$(curl -fs "$SERVER/api/projects/$PID" "${AUTH[@]}" | jq -r '[.checkpoint_setting, .checkpoint_storage] | join(",")')" = "auto,local"
curl -fs -X PATCH "$SERVER/api/admin/projects/$PID" "${AUTH[@]}" -d '{"checkpoints":"server"}' >/dev/null
check "the folder picks up the project's setting" test "$(alice -C "$A" space --json | jq -r .storage)" = server
BEFORE=$(parts "$A")
printf 'second' >> "$A/Docs/big.bin"
OUT=$(alice -C "$A" checkpoint -m "kept on the server" --json)
SCP=$(echo "$OUT" | jq -r .checkpoint.id)
check "the checkpoint went to the server" test "$(echo "$OUT" | jq .on_server)" = true
check "and added nothing to this computer's cache" test "$(parts "$A")" = "$BEFORE"
check "alice sees it in the server's list" test "$(alice -C "$A" checkpoints --server --json | jq --arg id "$SCP" '[.[] | select(.id==$id)] | length')" = 1
check "bob does not see alice's checkpoints" test "$(bob -C "$B" checkpoints --server --json | jq length)" = 0
check "bob cannot fetch it by id" test "$(curl -s -o /dev/null -w '%{http_code}' "$SERVER/api/projects/$PID/checkpoints/$SCP" -H "authorization: Bearer $BTOK")" = 404
USAGE=$(curl -fs "$SERVER/api/admin/checkpoints" "${AUTH[@]}")
echo "   usage: $(echo "$USAGE" | jq -c '.[0] | {who: .user.name, checkpoints, stored}')"
check "admins see how much space it takes" test "$(echo "$USAGE" | jq '.[0].stored')" -gt 0
check "storage check still passes" test "$(alice admin check --json | jq .ok)" = true
rm "$A/.mp/checkpoints/$SCP.json"
printf 'third' >> "$A/Docs/big.bin"
alice -C "$A" rewind "$SCP" Docs/big.bin >/dev/null
check "after losing the local record, rewind restores from the server" bash -c "[ \"\$(tail -c 6 '$A/Docs/big.bin')\" = second ] && [ \$(stat -c %s '$A/Docs/big.bin') = 6000006 ]"
ALICE_ID=$(curl -fs "$SERVER/api/me" "${AUTH[@]}" | jq -r '.id // .user.id')
R=$(curl -fs -X POST "$SERVER/api/admin/checkpoints/clear" "${AUTH[@]}" -d "{\"user\":\"$ALICE_ID\",\"project\":\"$PID\"}")
check "an admin can clear a person's checkpoints ($(echo "$R" | jq .removed) removed)" test "$(echo "$R" | jq .removed)" -ge 1
check "and the list is empty afterwards" test "$(alice -C "$A" checkpoints --server --json | jq length)" = 0
curl -fs -X PATCH "$SERVER/api/admin/projects/$PID" "${AUTH[@]}" -d '{"checkpoints":"auto"}' >/dev/null
alice -C "$A" space >/dev/null
alice -C "$A" discard . >/dev/null

step "Automatic checkpoints are off unless a project or a person turns them on"
check "off for a new project" test "$(curl -fs "$SERVER/api/projects/$PID" "${AUTH[@]}" | jq .auto_checkpoints)" = false
check "off in alice's folder" test "$(alice -C "$A" space --json | jq .automatic)" = false
curl -fs -X PATCH "$SERVER/api/admin/projects/$PID" "${AUTH[@]}" -d '{"auto_checkpoints":true}' >/dev/null
check "a project admin can turn them on for everyone" test "$(alice -C "$A" space --json | jq .automatic)" = true
check "a person can still turn them off for their folder" test "$(alice -C "$A" space --auto-checkpoints off --json | jq .automatic)" = false
curl -fs -X PATCH "$SERVER/api/admin/projects/$PID" "${AUTH[@]}" -d '{"auto_checkpoints":false}' >/dev/null
alice -C "$A" space --auto-checkpoints project >/dev/null
bob -C "$B" space --auto-checkpoints on >/dev/null

step "Storage locations: a project's data moves while people keep working"
AUTH=(-H "authorization: Bearer $TOKEN" -H 'content-type: application/json')
check "the server lists main and second" test "$(curl -fs "$SERVER/api/admin/storage" "${AUTH[@]}" | jq -c '[.locations[].name]')" = '["main","second"]'
NEWP=$(curl -fs -X POST "$SERVER/api/orgs/$ORG_ID/projects" "${AUTH[@]}" -d '{"name":"Tidepool","storage":"second"}')
check "a new project can start in another location" test "$(echo "$NEWP" | jq -r .storage)" = second
check "an unknown location is refused" test "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$SERVER/api/orgs/$ORG_ID/projects" "${AUTH[@]}" -d '{"name":"Nowhere","storage":"moon"}')" = 422
check "Emberfall starts in main" test "$(curl -fs "$SERVER/api/projects/$PID" "${AUTH[@]}" | jq -r .storage)" = main
MV=$(curl -fs -X POST "$SERVER/api/admin/projects/$PID/storage" "${AUTH[@]}" -d '{"to":"second"}')
check "a move starts" test "$(echo "$MV" | jq -r .status)" = running
for i in $(seq 1 120); do ST=$(curl -fs "$SERVER/api/admin/projects/$PID/storage" "${AUTH[@]}"); [ "$(echo "$ST" | jq -r .move.status)" != running ] && break; sleep 0.5; done
echo "   move: $(echo "$ST" | jq -c '{storage, previous, move: (.move | {status, total_chunks, copied_chunks, copied_bytes, error})}')"
check "the move finished" test "$(echo "$ST" | jq -r .move.status)" = done
check "Emberfall is now in second, nothing left mid-move" test "$(echo "$ST" | jq -c '[.storage, .previous]')" = '["second",null]'
check "every part was copied once, into the second location's folder" test "$(find "$WORK/blobs2/chunks" -type f | wc -l)" = "$(echo "$ST" | jq .move.copied_chunks)"
check "storage check passes after the move" test "$(alice admin check --json | jq .ok)" = true
head -c 300000 /dev/urandom > "$A/Docs/after_move.bin"
BEFORE=$(find "$WORK/blobs2/chunks" -type f | wc -l)
alice -C "$A" publish -m "First file after moving storage" >/dev/null
check "new uploads go to the new location" test "$(find "$WORK/blobs2/chunks" -type f | wc -l)" -gt "$BEFORE"
bob -C "$B" get-latest >/dev/null
check "bob gets it from the new location" same "$A/Docs/after_move.bin" "$B/Docs/after_move.bin"
curl -fs "$SERVER/api/projects/$PID/download?branch=$MAIN&path=Art/Hero/hero_body.fbx" -H "authorization: Bearer $TOKEN" -o "$WORK/dl2.bin"
check "old files download from the new location" same "$WORK/dl2.bin" "$A/Art/Hero/hero_body.fbx"
check "only server admins can move storage" test "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$SERVER/api/admin/projects/$PID/storage" -H "authorization: Bearer $BTOK" -H 'content-type: application/json' -d '{"to":"main"}')" = 403
check "reclaim space reports per location" test "$(curl -fs "$SERVER/api/admin/storage/gc" "${AUTH[@]}" | jq '.by_location | type')" = '"array"'

step "Moving a synced folder keeps everything"
EDITED_BEFORE=$(alice -C "$A" status --json | jq '[.status.new, .status.edited, .status.deleted] | map(length) | add')
A2="$WORK/moved/alice/Emberfall"
R=$(alice -C "$A" move-folder "$A2" --json)
check "moved by renaming on the same drive" test "$(echo "$R" | jq -r .method)" = renamed
check "the old folder is gone" test ! -e "$A"
check "the new folder has the files" same "$A2/Art/Hero/hero_body.fbx" "$B/Art/Hero/hero_body.fbx"
check "nothing looks changed after the move" test "$(alice -C "$A2" status --json | jq '[.status.new, .status.edited, .status.deleted] | map(length) | add')" = "$EDITED_BEFORE"
check "the list of synced folders points at the new place" bash -c "MP_CONFIG_DIR='$WORK/cfg-alice' '$BIN/mp' workspaces --json | grep -q moved"
mkdir -p "$WORK/occupied" && touch "$WORK/occupied/x"
check "moving into a folder with files is refused" bash -c "! MP_CONFIG_DIR='$WORK/cfg-alice' '$BIN/mp' -C '$A2' move-folder '$WORK/occupied' >/dev/null 2>&1"
check "moving into another synced folder is refused" bash -c "! MP_CONFIG_DIR='$WORK/cfg-bob' '$BIN/mp' -C '$B' move-folder '$C/inside' >/dev/null 2>&1"
check "and bob's folder stayed where it was" test -e "$B/Art/Hero/hero_body.fbx"
A="$A2"

step "Daemon: local API, cross-account projects, selective sync, origin guard"
DPORT=$((PORT+27))
MP_CONFIG_DIR="$WORK/cfg-bob" MP_DEVICE=bob-pc "$BIN/mp" daemon --listen "127.0.0.1:$DPORT" --interval 5 > "$WORK/daemon.log" 2>&1 &
DPID=$!
for i in $(seq 1 50); do curl -fs "http://127.0.0.1:$DPORT/local/health" >/dev/null && break; sleep 0.2; done
D="http://127.0.0.1:$DPORT/local"
check "daemon health lists bob's 2 folders" test "$(curl -fs "$D/health" | jq .workspaces)" = 2
check "projects across accounts include Emberfall with 2 folders here" test "$(curl -fs "$D/projects" | jq '.accounts[0].projects[] | select(.project.name=="Emberfall") | .workspaces | length')" = 2
WID=$(curl -fs "$D/workspaces" | jq -r --arg r "$B" '.[] | select(.root==$r) | .id')
check "folder tree has Art, Audio, Docs" test "$(curl -fs "$D/workspaces/$WID/folders" | jq -c '[.children[].name] | map(select(. == "Art" or . == "Audio" or . == "Docs"))')" = '["Art","Audio","Docs"]'
curl -fs -X PUT "$D/workspaces/$WID/sync" -H 'content-type: application/json' -d '{"include":[],"exclude":["Audio"]}' >/dev/null
check "excluding Audio through the daemon removed it" test ! -e "$B/Audio/wolf_howl.wav"
check "the tree shows Audio as not on this computer" test "$(curl -fs "$D/workspaces/$WID/folders" | jq -r '.children[] | select(.name=="Audio") | .included')" = none
curl -fs -X PUT "$D/workspaces/$WID/sync" -H 'content-type: application/json' -d '{"include":[],"exclude":[]}' >/dev/null
check "Audio is back" test -e "$B/Audio/wolf_howl.wav"
printf 'bob daemon edit' >> "$B/Art/Hero/hero_body.fbx"
check "status shows the edit" test "$(curl -fs "$D/workspaces/$WID/status" | jq '.edited | length')" = 1
check "a foreign web page is refused" test "$(curl -s -o /dev/null -w '%{http_code}' -H 'Origin: https://evil.example' "$D/workspaces")" = 403
check "the server's own web UI is allowed" test "$(curl -s -o /dev/null -w '%{http_code}' -H "Origin: $SERVER" "$D/workspaces")" = 200
check "web pages cannot read sign-in tokens" test "$(curl -s -o /dev/null -w '%{http_code}' -H "Origin: $SERVER" "$D/accounts/$(jq -r '.accounts[0].id' "$WORK/cfg-bob/accounts.json")/session")" = 403
sleep 7
check "the background pass claimed the edited model" test "$(bob -C "$B" claims --json | jq '[.[] | select(.path=="Art/Hero/hero_body.fbx" and .user.name=="Bob")] | length')" = 1
check "and took an automatic checkpoint where bob turned them on" test "$(curl -fs "$D/workspaces/$WID/checkpoints" | jq '[.[] | select(.auto)] | length')" -ge 1
check "but none in bob's other folder, which has edits and the default setting" test "$(bob -C "$C" checkpoints --json | jq '[.[] | select(.auto)] | length')" = 0
R=$(curl -fs -X POST "$D/workspaces/$WID/publish" -H 'content-type: application/json' -d '{"message":"Published from the app"}')
check "publish through the daemon" test "$(echo "$R" | jq '.change.number')" -gt 0
check "suggest ignore choices through the daemon" test "$(curl -fs "$D/workspaces/$WID/ignore/suggest?path=Art/Hero/hero_body.fbx" | jq length)" -ge 3

step "Desktop screens: files, history, branches, claims and drafts through the daemon"
APP=(-H 'Origin: tauri://localhost' -H 'content-type: application/json')
BMAIN=$(curl -fs "${APP[@]}" "$D/workspaces/$WID/server/branches" | jq -r '.[] | select(.name=="Main") | .id')
check "the app lists the folder's files through the daemon" test "$(curl -fs "${APP[@]}" "$D/workspaces/$WID/server/branches/$BMAIN/files?prefix=Art/" | jq 'length')" -ge 2
check "and its history" test "$(curl -fs "${APP[@]}" "$D/workspaces/$WID/server/changes?branch=$BMAIN&limit=5" | jq 'length')" = 5
check "a web page cannot use the pass-through" test "$(curl -s -o /dev/null -w '%{http_code}' -H 'Origin: http://localhost:5173' "$D/workspaces/$WID/server/branches")" = 403
check "paths cannot climb out of the project" test "$(curl -s -o /dev/null -w '%{http_code}' "${APP[@]}" "$D/workspaces/$WID/server/x/../../me")" != 200
check "server refusals come back as they are" test "$(curl -s -o /dev/null -w '%{http_code}' "${APP[@]}" "$D/workspaces/$WID/server/changes/99999")" = 404
curl -fs "${APP[@]}" -X POST "$D/workspaces/$WID/server/claims" -d "{\"branch\":\"$BMAIN\",\"paths\":[\"Docs/notes.txt\"],\"device\":\"Desktop app\"}" >/dev/null
check "claiming from the app" test "$(bob -C "$B" claims --json | jq '[.[] | select(.path=="Docs/notes.txt" and .user.name=="Bob")] | length')" = 1
curl -fs "${APP[@]}" -X POST "$D/workspaces/$WID/server/claims/release" -d "{\"branch\":\"$BMAIN\",\"paths\":[\"Docs/notes.txt\"]}" >/dev/null
check "releasing from the app" test "$(bob -C "$B" claims --json | jq '[.[] | select(.path=="Docs/notes.txt")] | length')" = 0
printf 'app draft\n' > "$B/Docs/app-draft.txt"
curl -fs "${APP[@]}" -X POST "$D/workspaces/$WID/draft" -d '{"title":"From the app"}' >/dev/null
DID=$(curl -fs "${APP[@]}" "$D/workspaces/$WID/server/drafts" | jq -r '.[] | select(.title=="From the app") | .id')
check "the draft is listed" test -n "$DID"
rm "$B/Docs/app-draft.txt"
check "switching branches is refused while there is unpublished work" bash -c "printf x > '$B/Docs/tmp-dirty.txt'; test \"\$(curl -s -o /dev/null -w '%{http_code}' -H 'content-type: application/json' -X POST '$D/workspaces/$WID/branch' -d '{\"branch_id\":\"$BMAIN\"}')\" = 409; rm '$B/Docs/tmp-dirty.txt'"
NB=$(curl -fs "${APP[@]}" -X POST "$D/workspaces/$WID/server/branches" -d "{\"name\":\"app-branch\",\"from\":\"$BMAIN\",\"shares_claims\":true}" | jq -r .id)
R=$(curl -fs "${APP[@]}" -X POST "$D/workspaces/$WID/branch" -d "{\"branch_id\":\"$NB\"}")
check "the app switches the folder to a new branch" test "$(echo "$R" | jq -r .workspace.branch_name)" = app-branch
curl -fs "${APP[@]}" -X POST "$D/workspaces/$WID/drafts/$DID/restore" >/dev/null
check "picking up the draft writes its file" grep -q "app draft" "$B/Docs/app-draft.txt"
curl -fs -X POST "$D/workspaces/$WID/publish" -H 'content-type: application/json' -d '{"message":"Draft finished on a branch"}' >/dev/null
check "merging the branch from the app" test "$(curl -fs "${APP[@]}" -X POST "$D/workspaces/$WID/server/branches/$NB/merge" -d '{"message":"Merge app-branch"}' | jq '.change.number')" -gt 0
curl -fs "${APP[@]}" -X DELETE "$D/workspaces/$WID/server/drafts/$DID" >/dev/null
check "deleting the draft" test "$(curl -fs "${APP[@]}" "$D/workspaces/$WID/server/drafts" | jq '[.[] | select(.id=="'"$DID"'")] | length')" = 0
curl -fs "${APP[@]}" -X POST "$D/workspaces/$WID/branch" -d "{\"branch_id\":\"$BMAIN\"}" >/dev/null
check "and back on Main the merged file is there" grep -q "app draft" "$B/Docs/app-draft.txt"

step "Notifications: others' changes and requests to release reach the desktop app"
N0=$(curl -fs "$D/notifications" | jq .last)
A2="$WORK/alice/Emberfall-notify"
alice connect Emberfall "$A2" >/dev/null
printf 'from alice\n' > "$A2/Docs/for-bob.txt"
alice -C "$A2" publish -m "A note for Bob" Docs/for-bob.txt >/dev/null
bob -C "$B" claim Docs/notes.txt >/dev/null
curl -fs -X POST "$SERVER/api/projects/$PID/claims/nudge" -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' -d "{\"branch\":\"$MAIN\",\"path\":\"Docs/notes.txt\"}" >/dev/null
for i in $(seq 1 30); do [ "$(curl -fs "$D/notifications?after=$N0" | jq '.notifications | length')" -ge 2 ] && break; sleep 0.5; done
NOTES=$(curl -fs "$D/notifications?after=$N0")
check "bob is told alice published to his branch" bash -c "echo '$(echo "$NOTES" | jq -r '[.notifications[] | select(.kind=="newer")][0].body')' | grep -q 'published change [0-9]*: A note for Bob'"
check "and that alice is waiting for his file" bash -c "echo '$(echo "$NOTES" | jq -r '[.notifications[] | select(.kind=="nudge")][0].title')' | grep -q 'is waiting for notes.txt'"
check "the tray count says bob's folder is behind" test "$(curl -fs "$D/behind" | jq --arg w "$WID" '.[$w]')" -ge 1
curl -fs -X PUT "$D/notifications/settings" -H 'content-type: application/json' -d '{"newer":false,"nudges":true}' >/dev/null
printf 'again\n' >> "$A2/Docs/for-bob.txt"
alice -C "$A2" publish -m "Another note" Docs/for-bob.txt >/dev/null
sleep 2
check "turning off change notifications stops them" test "$(curl -fs "$D/notifications?after=$(echo "$NOTES" | jq .last)" | jq '[.notifications[] | select(.kind=="newer")] | length')" = 0
bob -C "$B" release Docs/notes.txt >/dev/null
kill $DPID 2>/dev/null || true

step "Thumbnails: pictures of images and Blender files"
mkdir -p "$A2/Art/Pictures"
python3 - "$A2/Art/Pictures" <<'PY'
import struct, sys, zlib
d = sys.argv[1]
w, h = 300, 200
NUL = bytes([0])
raw = b"".join(NUL + bytes([200, 40, 30]) * w for _ in range(h))
chunk = lambda t, b: struct.pack(">I", len(b)) + t + b + struct.pack(">I", zlib.crc32(t + b) & 0xffffffff)
sig = bytes([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
open(f"{d}/red.png", "wb").write(sig + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))
# A Blender file with a 2x1 preview (TEST block), 8-byte pointers, little endian.
body = struct.pack("<II", 2, 1) + bytes([255, 0, 0, 255, 0, 0, 255, 255])
blend = b"BLENDER-v405" + b"TEST" + struct.pack("<I", len(body)) + NUL * 8 + struct.pack("<II", 0, 1) + body + b"ENDB" + NUL * 20
open(f"{d}/rig.blend", "wb").write(blend)
open(f"{d}/mesh.fbx", "wb").write(b"Kaydara FBX Binary  " + NUL * 101)
PY
alice -C "$A2" publish -m "Pictures" Art/Pictures >/dev/null
hash_of() { curl -fs "$SERVER/api/projects/$PID/branches/$MAIN/files?prefix=Art/Pictures/" -H "authorization: Bearer $TOKEN" | jq -r --arg p "$1" '.[] | select(.path==$p) | .file_hash'; }
thumb() { curl -s -o "$WORK/thumb.jpg" -D "$WORK/thumb.h" -w '%{http_code}' "$SERVER/api/projects/$PID/thumbs/$1${2:-}" -H "authorization: Bearer $TOKEN"; }
PNG=$(hash_of Art/Pictures/red.png); BL=$(hash_of Art/Pictures/rig.blend); FBX=$(hash_of Art/Pictures/mesh.fbx)
for i in $(seq 1 40); do [ "$(thumb "$PNG")" = 200 ] && [ "$(thumb "$BL")" = 200 ] && break; sleep 0.5; done
check "the PNG is drawn" test "$(thumb "$PNG")" = 200
check "as a JPEG of the right shape" bash -c "grep -qi 'content-type: image/jpeg' '$WORK/thumb.h' && grep -qi 'x-image-size: 300x200' '$WORK/thumb.h'"
check "and a large preview" test "$(thumb "$PNG" '?size=large')" = 200
check "the Blender file's own preview is used" bash -c "test \"\$(curl -s -o /dev/null -D - '$SERVER/api/projects/$PID/thumbs/$BL' -H 'authorization: Bearer $TOKEN' | grep -i x-image-size | tr -d '[:cntrl:]')\" = 'x-image-size: 2x1'"
check "a model has no picture (404)" test "$(thumb "$FBX")" = 404
check "a hash from outside the project is refused" test "$(thumb "$(printf 'a%.0s' $(seq 1 64))")" = 404
thumb "$PNG" >/dev/null
check "thumbnails are cached for good" grep -qi 'cache-control: private, max-age=31536000, immutable' "$WORK/thumb.h"

step "Backups: back up, restore into a fresh database and storage, and serve from it"
AUTH=(-H "authorization: Bearer $TOKEN" -H 'content-type: application/json')
wait_backup() { for i in $(seq 1 240); do B=$(curl -fs "$SERVER/api/admin/backups" "${AUTH[@]}"); [ "$(echo "$B" | jq .running)" = false ] && [ "$(echo "$B" | jq -r '.runs[0].status // "none"')" != running ] && return 0; sleep 0.5; done; return 1; }
wait_backup || true
curl -s -o /dev/null -X POST "$SERVER/api/admin/backups/run" "${AUTH[@]}"
sleep 1; wait_backup
B=$(curl -fs "$SERVER/api/admin/backups" "${AUTH[@]}")
echo "   last run: $(echo "$B" | jq -c '.runs[0] | {status, snapshot, total_chunks, new_chunks, db_bytes, error}')"
check "the backup finished" test "$(echo "$B" | jq -r '.runs[0].status')" = done
check "a complete backup is on the target" test -e "$WORK/backups/db/$(echo "$B" | jq -r '.runs[0].snapshot')/manifest.json"
check "file data was copied" test "$(find "$WORK/backups/chunks" -type f | wc -l)" -gt 0
curl -s -o /dev/null -X POST "$SERVER/api/admin/backups/run" "${AUTH[@]}"
sleep 1; wait_backup
B=$(curl -fs "$SERVER/api/admin/backups" "${AUTH[@]}")
check "a second backup copies no file data again" test "$(echo "$B" | jq '.runs[0].new_chunks')" = 0
check "only server admins see backups" test "$(curl -s -o /dev/null -w '%{http_code}' "$SERVER/api/admin/backups" -H "authorization: Bearer $(jq -r '.accounts[0].token' "$WORK/cfg-bob/accounts.json")")" = 403
RDB="mp_e2e_restore_$(date +%s)"
psql "$DB_ADMIN" -qc "CREATE DATABASE $RDB" >/dev/null
R_ENV=(env DATABASE_URL="${psql_url}/$RDB" MP_STORAGE=local MP_STORAGE_PATH="$WORK/restored" MP_STORAGE_LOCATIONS="second=local:$WORK/restored2" MP_BACKUP_TARGET= RUST_LOG=warn)
"${R_ENV[@]}" "$BIN/mp-server" restore --from "local:$WORK/backups" > "$WORK/restore.txt" 2>&1 || { cat "$WORK/restore.txt"; fail "restore failed"; }
sed 's/^/   | /' "$WORK/restore.txt"
check "restoring into a database with people in it is refused" bash -c "! env DATABASE_URL='${psql_url}/$RDB' MP_STORAGE=local MP_STORAGE_PATH='$WORK/restored' RUST_LOG=warn '$BIN/mp-server' restore --from 'local:$WORK/backups' >/dev/null 2>&1"
RPORT=$((PORT+5))
"${R_ENV[@]}" MP_LISTEN="127.0.0.1:$RPORT" "$BIN/mp-server" > "$WORK/restored-server.log" 2>&1 &
RPID=$!
for i in $(seq 1 50); do curl -fs "http://127.0.0.1:$RPORT/api/health" >/dev/null && break; sleep 0.2; done
RTOK=$(curl -fs -X POST "http://127.0.0.1:$RPORT/api/auth/login" -H 'content-type: application/json' -d "{\"email\":\"alice@example.com\",\"password\":\"$PASS_A\"}" | jq -r .token)
check "alice signs in to the restored server" test -n "$RTOK" -a "$RTOK" != null
check "it has the same history" test "$(curl -fs "http://127.0.0.1:$RPORT/api/projects/$PID/changes" -H "authorization: Bearer $RTOK" | jq length)" = "$(curl -fs "$SERVER/api/projects/$PID/changes" -H "authorization: Bearer $TOKEN" | jq length)"
check "its storage check passes" test "$(curl -fs -X POST "http://127.0.0.1:$RPORT/api/admin/storage/check" -H "authorization: Bearer $RTOK" -H 'content-type: application/json' -d '{}' | jq .ok)" = true
curl -fs "http://127.0.0.1:$RPORT/api/projects/$PID/download?branch=$MAIN&path=Art/Hero/hero_body.fbx" -H "authorization: Bearer $RTOK" -o "$WORK/restored.fbx"
curl -fs "$SERVER/api/projects/$PID/download?branch=$MAIN&path=Art/Hero/hero_body.fbx" -H "authorization: Bearer $TOKEN" -o "$WORK/original.fbx"
check "a file downloads byte for byte from the restored copy" same "$WORK/restored.fbx" "$WORK/original.fbx"
kill $RPID 2>/dev/null || true

echo
echo "All $STEP steps passed."
