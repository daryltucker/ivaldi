#!/bin/bash
set -e
unset ANTIGRAVITY_AGENT
unset AI_AGENT

# Setup sandbox in tests/
mkdir -p tests/middleware_verify
cd tests/middleware_verify

# GIT AWARENESS SETUP
# Create a local .gitignore to ensure we have rules active for this test
echo "*.tmp" > .gitignore
touch ignore_me.tmp

# PERMISSION FIXER SETUP
mkdir -p readonly_dir
if [ -d "readonly_dir" ]; then
    chmod 700 readonly_dir # Ensure writable so we can setup if needed
fi

echo "Building server..."
# Go back to root to build
cd ../..
cargo build -p ivaldi-server --quiet
SERVER_BIN="./target/debug/ivaldi-server"

echo "=== TEST 1: GitAwareness (Middleware Pre-Flight) ==="
# Should warn about ignored file
REQ1='{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "write_file", "arguments": { "path": "tests/middleware_verify/ignore_me.tmp", "content": "foo", "overwrite": true } } }'
echo "$REQ1" | $SERVER_BIN > output1.json

if grep -q "git_status" output1.json; then
    echo "✅ GitAwareness advisory found (JSON structured)"
else
    echo "❌ GitAwareness advisory MISSING or not structured"
    cat output1.json
    exit 1
fi

echo "=== TEST 2: PermissionFixer (Contextual Density) ==="
# Should fail with EACCES and give rich context about PARENT (since we can't create file)
if [ -d "tests/middleware_verify/readonly_dir" ]; then
    chmod 700 tests/middleware_verify/readonly_dir # Restore perms to allow deletion
    trash tests/middleware_verify/readonly_dir
fi
mkdir -p tests/middleware_verify/readonly_dir
chmod 000 tests/middleware_verify/readonly_dir # NO Access
ls -ld tests/middleware_verify/readonly_dir

REQ2='{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "write_file", "arguments": { "path": "tests/middleware_verify/readonly_dir/new_file.txt", "content": "bar", "overwrite": true } } }'
echo "$REQ2" | $SERVER_BIN > output2.json
cat output2.json | jq .

# Expecting "focus": "parent" because file doesn't exist
if grep -q "parent" output2.json && grep -q "uid" output2.json; then
    echo "✅ PermissionFixer rich context found (Parent Focus, UID)"
else
    echo "❌ PermissionFixer missing rich context"
    # Debug info
    echo "Output content:"
    cat output2.json
    exit 1
fi

# Cleanup

echo "Cleaning up..."
chmod 700 tests/middleware_verify/readonly_dir
# Ensure we trash the directory we created
if [ -d "tests/middleware_verify" ]; then
    trash tests/middleware_verify
else
    echo "Warning: tests/middleware_verify not found for cleanup"
fi
echo "ALL TESTS PASSED"
