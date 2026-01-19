#!/bin/bash
set -e

# === Agent-Led Verification Phase 1: Blind Discovery ===
# Purpose: Verify server health and tool exposure.

echo "Building server..."
cargo build -p ivaldi-server --quiet
SERVER_BIN="./target/debug/ivaldi-server"

echo "=== TEST 1: Blind Discovery (list_tools) ==="
REQ='{"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}'
output=$(echo "$REQ" | $SERVER_BIN)

echo "$output" > output_discovery.json

# Verification Logic
if grep -q "write_file" output_discovery.json; then
    echo "✅ Tool 'write_file' exposed."
else
    echo "❌ Tool 'write_file' MISSING."
    exit 1
fi


# === Agent-Led Verification Phase 2: Ingest & Query Loop ===
echo "=== TEST 2: Wisdom Layer (Ingest & Query) ==="

# 1. Setup Environment
export IVALDI_ADT_ENABLED=1
export VECDB_CONFIG="$PWD/../vecdb-mcp/tests/fixtures/config.toml"
echo "Configuration: $VECDB_CONFIG"

# 2. Build Tools
echo "Building vecdb-cli..."
# Ensure we build in the correct workspace
pushd ../vecdb-mcp > /dev/null
cargo build -p vecdb-cli --quiet
popd > /dev/null

VECDB_BIN="../vecdb-mcp/target/debug/vecdb"

# 3. Prepare Collection (ivaldi_ops)

echo "Creating ivaldi_ops collection..."
$VECDB_BIN create ivaldi_ops --dimension 384 || true # Ignore if exists (or explicit recreate?)
$VECDB_BIN list || echo "Failed to list collections"

# 4. Execute Server Operation (write_file)
echo "Executing write_file with ADT logging..."
REQ_WRITE='{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "write_file", "arguments": { "path": "tests/middleware_verify/wisdom_test.txt", "content": "Wisdom is key.", "overwrite": true } } }'

echo "$REQ_WRITE" | $SERVER_BIN > output_wisdom.json
cat output_wisdom.json

# 5. Verify Wisdom Log
echo "Verifying Wisdom Log in vecdb..."
sleep 2 # Allow async ingest to settle

# Search for the tool execution
SEARCH_OUT=$($VECDB_BIN search "write_file" --collection ivaldi_ops)
echo "$SEARCH_OUT"

if echo "$SEARCH_OUT" | grep -q "write_file"; then
    echo "✅ WisdomEntry found in vecdb!"
else
    echo "❌ WisdomEntry NOT found."
    # Dump log for debugging
    # echo "Server Output:"
    # cat output_wisdom.json
    exit 1
fi

echo "✅ Phase 2: Ingest & Query Loop PASSED"

