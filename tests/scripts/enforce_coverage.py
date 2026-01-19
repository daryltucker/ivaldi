#!/usr/bin/env python3
import json
import subprocess
import sys
import os

def main():
    print("🚀 Running Coverage Enforcement...")
    
    # Run cargo llvm-cov to get JSON report
    try:
        result = subprocess.run(
            ["cargo", "llvm-cov", "nextest", "--workspace", "--json"],
            capture_output=True,
            text=True,
            check=True
        )
    except subprocess.CalledProcessError as e:
        print(f"❌ Coverage measurement failed:\n{e.stderr}")
        sys.exit(1)

    data = json.loads(result.stdout)
    
    # Filter for files in src/ directories (ignoring tests/ build.rs etc)
    # The structure of llvm-cov JSON depends on the version
    # data['data'][0]['files'] usually contains the file info
    
    files_with_zero_coverage = []
    total_coverage = 0
    file_count = 0

    if 'data' not in data or not data['data']:
        print("❌ Invalid coverage data format")
        sys.exit(1)

    for item in data['data']:
        for file_info in item.get('files', []):
            filename = file_info['filename']
            
            # Only check source files in src/
            if '/src/' not in filename or filename.endswith('.rs') is False:
                continue
            
            # Ignore auto-generated or external files if any
            if 'target/' in filename:
                continue

            summary = file_info.get('summary', {})
            lines = summary.get('lines', {})
            percent = lines.get('percent', 0)
            
            file_count += 1
            total_coverage += percent

            if percent == 0:
                files_with_zero_coverage.append((filename, percent))
                print(f"❌ {filename}: {percent}% (ZERO COVERAGE)")
            elif percent < 50:
                print(f"⚠️ {filename}: {percent}% (Low coverage)")
            else:
                pass # print(f"✅ {filename}: {percent}%")

    if files_with_zero_coverage:
        print(f"\n🛑 FAILURE: {len(files_with_zero_coverage)} files have 0% coverage.")
        print("All source files must have at least one test covering them.")
        sys.exit(1)
    
    if file_count > 0:
        avg = total_coverage / file_count
        print(f"\n✨ Average coverage: {avg:.2f}%")
        print("✅ Coverage enforcement passed.")
    else:
        print("\n❓ No source files found to audit.")

if __name__ == "__main__":
    main()
