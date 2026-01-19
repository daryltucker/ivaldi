#!/usr/bin/env python3
import os
import sys
import json
import subprocess
import tempfile
import time
import shutil

# ANSI colors
GREEN = "\033[92m"
RED = "\033[91m"
RESET = "\033[0m"

def log_pass(msg):
    print(f"{GREEN}✓ {msg}{RESET}")

def log_fail(msg):
    print(f"{RED}✗ {msg}{RESET}")
    sys.exit(1)

def main():
    # 1. Setup Sandbox
    sandbox = tempfile.mkdtemp(prefix="ivaldi_tier3_")
    print(f"Running Tier 3 Test in Sandbox: {sandbox}")
    
    # Locate Binary
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    server_bin = os.path.join(repo_root, "target/debug/ivaldi-server")
    
    if not os.path.exists(server_bin):
        log_fail(f"Server binary not found at {server_bin}. Run 'cargo build' first.")

    # 2. Start Server Process
    process = subprocess.Popen(
        [server_bin],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=sys.stderr, # Pass stderr through to console for debugging
        text=True,
        bufsize=0 # Unbuffered
    )

    def send_request(method, params, req_id):
        req = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params
        }
        json_str = json.dumps(req) + "\n"
        process.stdin.write(json_str)
        process.stdin.flush()
        
        # Read response
        line = process.stdout.readline()
        if not line:
            log_fail("Server closed connection unexpectedly")
        return json.loads(line)

    try:
        # 3. Initialize
        print("Initializing...")
        # Note: Ivaldi implementation might not strictly require initialize for tools, 
        # but let's assume we skip straight to tools/call for this simple test harness
        # or implement a minimal init if needed. Current ivaldi implementation handles tools/call directly.
        
        # 4. Scenario: Write -> Edit -> Undo
        test_file = os.path.join(sandbox, "test.txt")
        
        # Action A: Write File
        print("Testing: write_file...")
        res = send_request("tools/call", {
            "name": "write_file",
            "arguments": {
                "path": test_file,
                "content": "Hello World\n",
                "overwrite": True
            }
        }, 1)
        
        if "error" in res:
            log_fail(f"write_file failed: {res['error']}")
        
        if not os.path.exists(test_file):
            log_fail("File was not created on disk")
        
        with open(test_file, 'r') as f:
            if f.read() != "Hello World\n":
                log_fail("File content mismatch after write")
        log_pass("write_file successful")

        # Action B: Edit File
        print("Testing: edit_file...")
        res = send_request("tools/call", {
            "name": "edit_file",
            "arguments": {
                "path": test_file,
                "grep": "^Hello",
                "replacement": "Goodbye",
                "overwrite": True
            }
        }, 2)

        if "error" in res:
            log_fail(f"edit_file failed: {res['error']}")
            
        with open(test_file, 'r') as f:
            content = f.read()
            if "Goodbye" not in content:
                log_fail(f"Edit failed. Content: {content}")
        log_pass("edit_file successful")

        # Action C: Undo
        print("Testing: undo...")
        res = send_request("tools/call", {
            "name": "undo",
            "arguments": {
                "path": sandbox # Undo usually takes project root or path context
            }
        }, 3)

        if "error" in res:
            log_fail(f"undo failed: {res['error']}")
            
        with open(test_file, 'r') as f:
            content = f.read()
            if content != "Hello World\n":
                log_fail(f"Undo failed. Content: {content}")
        log_pass("undo successful")
        
    finally:
        # Cleanup
        process.terminate()
        process.wait()
        shutil.rmtree(sandbox)
        print("Sandbox cleaned up.")

if __name__ == "__main__":
    main()
