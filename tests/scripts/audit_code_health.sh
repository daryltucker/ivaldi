#!/bin/bash
set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
PURPLE='\033[0;35m'
NC='\033[0m'

echo -e "${BLUE}=== Ivaldi Code Health Audit ===${NC}"

# Thresholds
INFO_LIMIT=200
WARN_LIMIT=300
ERROR_LIMIT=400
CRITICAL_LIMIT=500

STATUS=0

# Find all .rs files, ignoring target and other artifacts
# Using fd if available, else find
if command -v fd &> /dev/null; then
    FILES=$(fd -e rs -E target -E .git -E .ivaldi)
else
    FILES=$(find . -name "*.rs" -not -path "*/target/*" -not -path "*/.git/*" -not -path "*/.ivaldi/*")
fi

echo -e "Scanning $(echo "$FILES" | wc -l) files..."
echo ""

while IFS= read -r file; do
    LINES=$(wc -l < "$file")
    
    if [ "$LINES" -gt "$CRITICAL_LIMIT" ]; then
        echo -e "${PURPLE}[CRITICAL]${NC} $file: $LINES lines (> $CRITICAL_LIMIT)"
        STATUS=1
    elif [ "$LINES" -gt "$ERROR_LIMIT" ]; then
        echo -e "${RED}[ERROR]   ${NC} $file: $LINES lines (> $ERROR_LIMIT)"
        STATUS=1
    elif [ "$LINES" -gt "$WARN_LIMIT" ]; then
        echo -e "${YELLOW}[WARNING] ${NC} $file: $LINES lines (> $WARN_LIMIT)"
    elif [ "$LINES" -gt "$INFO_LIMIT" ]; then
        echo -e "${BLUE}[INFO]    ${NC} $file: $LINES lines (> $INFO_LIMIT)"
    fi
done <<< "$FILES"

echo ""
if [ $STATUS -eq 0 ]; then
    echo -e "${GREEN}Code health check passed.${NC}"
else
    echo -e "${RED}Code health check failed. Files exceed absolute limits.${NC}"
fi

exit $STATUS
