#!/usr/bin/env bash
# I7 end-to-end: fake Ollama + real TandemForge backend + IDE core client. Test-only, loopback only.
set -euo pipefail
OUT="${1:?output folder}"; mkdir -p "$OUT"
WORK="$(mktemp -d)"; trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$WORK"' EXIT
TOKEN="[REDACTED:secret] -c 'import secrets;print(secrets.token_urlsafe(32))')"
python3 /tmp/claude-0/i7-e2e/fake_ollama.py 11555 "$WORK/ollama.jsonl" & 
cd /home/user/TandemForge/packages/python
OLLAMA_BASE_URL=http://127.0.0.1:11555 TANDEMFORGE_API_TOKEN="$TOKEN" TANDEMFORGE_DB="$WORK/data/orchestrator.db" \
  TANDEMFORGE_PORT=8798 TANDEMFORGE_HOST=127.0.0.1 TANDEMFORGE_WARM_MODELS=0 TANDEMFORGE_P2PAI_NODE_DIR="$WORK/node" \
  /tmp/claude-0/tfvenv/bin/python server.py > "$WORK/backend.log" 2>&1 &
for _ in $(seq 60); do curl -sf http://127.0.0.1:8798/api/health >/dev/null && break; sleep 0.5; done
echo "engine.json before pairing: $(test -f "$WORK/data/engine.json" && echo present || echo absent)" | tee "$OUT/steps.txt"
# Owner connects the IDE (needs the desktop app's own token).
RESP="$(curl -sf -X POST -H "x-tandemforge-token: $TOKEN" -H 'Content-Type: application/json' -d '{"name":"P2P-AI IDE"}' http://127.0.0.1:8798/api/connected-apps)"
CODE="$(printf '%s' "$RESP" | python3 -c 'import json,sys;print(json.load(sys.stdin)["code"])')"
APP="$(printf '%s' "$RESP" | python3 -c 'import json,sys;print(json.load(sys.stdin)["app"]["id"])')"
echo "engine.json after pairing: $(cat "$WORK/data/engine.json" | python3 -c 'import json,sys;d=json.load(sys.stdin);print({"api":d["api"],"port":d["port"],"pid":"<pid>"})')" | tee -a "$OUT/steps.txt"
echo "engine.json mode: $(stat -c %a "$WORK/data/engine.json"); connected-apps.json mode: $(stat -c %a "$WORK/data/connected-apps.json")" | tee -a "$OUT/steps.txt"
echo "code stored in connected-apps.json: $(grep -c "$CODE" "$WORK/data/connected-apps.json" || true) (0 = only the digest)" | tee -a "$OUT/steps.txt"
# A pairing code must not open the owner's /api routes.
echo "pairing code on /api/connected-apps: HTTP $(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $CODE" http://127.0.0.1:8798/api/connected-apps)" | tee -a "$OUT/steps.txt"
echo "no credentials on /v1/models: HTTP $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8798/v1/models)" | tee -a "$OUT/steps.txt"
echo "browser Origin on /v1/models: HTTP $(curl -s -o /dev/null -w '%{http_code}' -H 'Origin: https://evil.example' -H "Authorization: Bearer $CODE" http://127.0.0.1:8798/v1/models)" | tee -a "$OUT/steps.txt"
# The IDE core end to end.
cd /tmp/claude-0/ide-i6/core
TF_ENGINE_DIR="$WORK/data" TF_CODE="$CODE" cargo test --test tandemforge_engine -- --ignored --nocapture 2>&1 | tee "$OUT/ide-core.txt" | grep -E "PASS|test result|panicked" || true
# Revoke: the code stops working at once and engine.json goes away.
curl -sf -X DELETE -H "x-tandemforge-token: $TOKEN" "http://127.0.0.1:8798/api/connected-apps/$APP" >/dev/null
echo "after revoke, code on /v1/models: HTTP $(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $CODE" http://127.0.0.1:8798/v1/models)" | tee -a "$OUT/steps.txt"
echo "engine.json after revoke: $(test -f "$WORK/data/engine.json" && echo present || echo absent)" | tee -a "$OUT/steps.txt"
# What reached the model: every chat pinned to the fake local Ollama.
python3 -c "
import json,sys
rows=[json.loads(l) for l in open('$WORK/ollama.jsonl')]
chats=[r for r in rows if r['path']=='/api/chat']
print('ollama /api/chat requests:',len(chats),'models:',sorted({r['body']['model'] for r in chats}))
t=[r for r in chats if any(m.get('tool_calls') for m in r['body']['messages'])]
print('tool-call arguments sent to Ollama as objects:',all(isinstance(c['function']['arguments'],dict) for r in t for m in r['body']['messages'] for c in (m.get('tool_calls') or [])), len(t))
" | tee -a "$OUT/steps.txt"
grep -c "$CODE" "$WORK/backend.log" | sed 's/^/pairing code in backend log (count): /' | tee -a "$OUT/steps.txt" || true
sed "s/$TOKEN/<token>/g; s/$CODE/<code>/g" "$WORK/backend.log" | grep -iE "engine|connected|error|warn" | head -40 > "$OUT/backend-excerpt.log" || true
