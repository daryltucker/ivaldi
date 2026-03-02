#!/usr/bin/env python3
# ═══════════════════════════════════════════════════════════════════
# TIER 4: STRESS TEST — Large Files & Complex Edits
# ═══════════════════════════════════════════════════════════════════

import os
import sys
import json
import subprocess
import tempfile
import shutil
import time

GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
RESET = "\033[0m"

PASS_COUNT = 0
FAIL_COUNT = 0

def log_pass(msg):
    global PASS_COUNT
    PASS_COUNT += 1
    print(f"{GREEN}✓ {msg}{RESET}")

def log_fail(msg):
    global FAIL_COUNT
    FAIL_COUNT += 1
    print(f"{RED}✗ {msg}{RESET}")

def generate_10mb_file(path):
    """Generate a file close to 10MB with identifiable sections."""
    print(f"    Generating 10MB file...")
    start_time = time.time()
    total_lines = 0
    with open(path, "w") as f:
        # 1. Header (3 lines)
        f.write("# START\n")
        f.write("def header():\n")
        f.write("    print('header')\n")
        total_lines += 3
        
        # 2. Filler (200,000 * 4 = 800,000 lines)
        # Each function is 4 lines
        for i in range(200000):
            f.write(f"def filler_{i}():\n")
            f.write(f"    x = {i}\n")
            f.write(f"    return x\n")
            f.write("\n")
            total_lines += 4
            
        # 3. Footer (2 lines)
        f.write("def footer():\n")
        f.write("    print('footer')\n")
        total_lines += 2
    
    end_time = time.time()
    size = os.path.getsize(path) / (1024 * 1024)
    print(f"    Generated {size:.2f} MB in {end_time - start_time:.2f}s (Total lines: {total_lines})")
    return total_lines

def main():
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    server_bin = os.path.join(repo_root, "target/debug/ivaldi-server")

    if not os.path.exists(server_bin):
        print(f"{RED}Server binary not found at {server_bin}. Run 'make build' first.{RESET}")
        sys.exit(1)

    sandbox = tempfile.mkdtemp(prefix="ivaldi_tier4_stress_")
    print(f"[T4-STRESS] Sandbox: {sandbox}")

    process = subprocess.Popen(
        [server_bin],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0,
    )

    def rpc(method, params, req_id):
        req = {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}
        process.stdin.write(json.dumps(req) + "\n")
        process.stdin.flush()
        line = process.stdout.readline()
        if not line:
            return None
        return json.loads(line)

    try:
        target_file = os.path.join(sandbox, "stress.py")
        total_lines = generate_10mb_file(target_file)

        # ── TEST 1: TOP Edit ─────────────────────────────────────
        print(f"\n{YELLOW}[T4.S1] Editing TOP...{RESET}")
        res = rpc("tools/call", {
            "name": "edit_file",
            "arguments": {
                "path": target_file,
                "from_line": 2,
                "to_line": 3,
                "replacement": "def new_header():\n    print('FIXED')"
            }
        }, 1)
        if res and "error" not in res:
            log_pass("TOP edit successful")
        else:
            log_fail(f"TOP edit failed: {res}")

        # ── TEST 2: MIDDLE Edit (Indentation Healing) ───────────
        print(f"\n{YELLOW}[T4.S2] Editing MIDDLE (Healing)...{RESET}")
        # Targeting line 400,001 (inside a filler function)
        # Filler starts at line 4 (1-indexed)
        # filler_i starts at line 4 + i*4
        # for i=100000, starts at 400,004.
        target_line = 400005 # x = 100000
        res = rpc("tools/call", {
            "name": "edit_file",
            "arguments": {
                "path": target_file,
                "from_line": target_line,
                "to_line": target_line,
                "replacement": "x = 'HEALED'"
            }
        }, 2)
        if res and "error" not in res:
            with open(target_file, "r") as f:
                content = f.readlines()
                line = content[target_line - 1]
                if line == "    x = 'HEALED'\n":
                    log_pass("MIDDLE Indentation Healing verified")
                else:
                    log_fail(f"Healing failed. Found: {repr(line)}")
        else:
            log_fail(f"MIDDLE edit failed: {res}")

        # ── TEST 3: BOTTOM Edit (Anchor Trimming) ────────────────
        print(f"\n{YELLOW}[T4.S3] Editing BOTTOM (Trimming)...{RESET}")
        # Last line of original file is line 'total_lines' (800005)
        # Line 'total_lines - 1' is 'def footer():'
        res = rpc("tools/call", {
            "name": "edit_file",
            "arguments": {
                "path": target_file,
                "from_line": total_lines,
                "to_line": total_lines,
                "replacement": "def footer():\n    print('FIXED_FOOTER')"
            }
        }, 3)
        if res and "error" not in res:
            with open(target_file, "r") as f:
                content = f.readlines()
                # If trimming worked, the 'def footer():' anchor should have been merged
                # Content should end with 'def footer():' then '    print('FIXED_FOOTER')'
                if content[-2] == "def footer():\n" and content[-1] == "    print('FIXED_FOOTER')\n":
                    log_pass("BOTTOM Anchor Trimming verified")
                else:
                    log_fail(f"Trimming failed. Tail:\n{content[-3:]}")
        else:
            log_fail(f"BOTTOM edit failed: {res}")

    finally:
        process.terminate()
        process.wait()
        shutil.rmtree(sandbox)

    if FAIL_COUNT > 0:
        sys.exit(1)

if __name__ == "__main__":
    main()
