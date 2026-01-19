#!/bin/bash
# Test script to verify ivaldi-server logging

echo "Testing ivaldi-server logging..."
echo "Current IVALDI_LOG setting in mcp_config.json:"
grep -A 2 "IVALDI_LOG" ~/.gemini/antigravity/mcp_config.json

echo ""
echo "Running ivaldi-server process:"
ps aux | grep ivaldi-server | grep -v grep

echo ""
echo "Note: Since ivaldi-server is launched by Antigravity IDE via stdio,"
echo "the logs go to stderr which the IDE captures."
echo ""
echo "To see logs, check:"
echo "1. Antigravity IDE's MCP server output panel"
echo "2. Or run: strace -p 366559 -e write 2>&1 | grep -i 'executing\|completed'"
echo "3. Or check ~/.gemini/antigravity/logs/ if IDE logs there"
