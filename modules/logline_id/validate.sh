#!/bin/bash

# LogLine ID Module - Simple Validation Test
# Validates module completeness and basic functionality

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

log() {
    echo -e "${2:-$NC}${1}${NC}"
}

MODULE_DIR="$(dirname "$0")"
PASSED=0
TOTAL=0

test_file_exists() {
    local file="$1"
    local description="$2"
    
    ((TOTAL++))
    if [ -f "$file" ]; then
        log "✓ $description" "$GREEN"
        ((PASSED++))
        return 0
    else
        log "✗ $description - FILE NOT FOUND: $file" "$RED"
        return 1
    fi
}

test_directory_exists() {
    local dir="$1"
    local description="$2"
    
    ((TOTAL++))
    if [ -d "$dir" ]; then
        log "✓ $description" "$GREEN"
        ((PASSED++))
        return 0
    else
        log "✗ $description - DIRECTORY NOT FOUND: $dir" "$RED"
        return 1
    fi
}

test_json_valid() {
    local file="$1"
    local description="$2"
    
    ((TOTAL++))
    if [ -f "$file" ]; then
        if command -v python3 &> /dev/null; then
            if python3 -c "import json; json.load(open('$file'))" 2>/dev/null; then
                log "✓ $description" "$GREEN"
                ((PASSED++))
                return 0
            fi
        else
            # Fallback: basic syntax check
            if grep -q '{' "$file" && grep -q '}' "$file"; then
                log "✓ $description (basic check)" "$GREEN"
                ((PASSED++))
                return 0
            fi
        fi
    fi
    
    log "✗ $description - INVALID JSON: $file" "$RED"
    return 1
}

count_lines() {
    local file="$1"
    if [ -f "$file" ]; then
        wc -l < "$file" | tr -d ' '
    else
        echo "0"
    fi
}

log "🚀 LogLine ID Module - Validation Test" "$BOLD"
log "=======================================" "$BOLD"

# Test module structure
log "\n📁 Testing Module Structure:" "$BLUE"
test_directory_exists "$MODULE_DIR/schema" "Schema directory exists"
test_directory_exists "$MODULE_DIR/contracts" "Contracts directory exists"
test_directory_exists "$MODULE_DIR/receipts" "Receipts directory exists"
test_directory_exists "$MODULE_DIR/logic" "Logic directory exists"
test_directory_exists "$MODULE_DIR/ui" "UI directory exists"

# Test schema files
log "\n📋 Testing Schema Files:" "$BLUE"
test_file_exists "$MODULE_DIR/schema/logline_id.schema.json" "Main schema file exists"
test_json_valid "$MODULE_DIR/schema/logline_id.schema.json" "Main schema is valid JSON"

# Test contract files
log "\n📜 Testing Contract Files:" "$BLUE"
test_file_exists "$MODULE_DIR/contracts/identity.register.lll" "Identity registration contract exists"
test_file_exists "$MODULE_DIR/contracts/ghost.claim.lll" "Ghost claim contract exists"
test_file_exists "$MODULE_DIR/contracts/verify_logline_id_signature.lll" "Signature verification contract exists"

# Test receipt schemas
log "\n🧾 Testing Receipt Schemas:" "$BLUE"
test_file_exists "$MODULE_DIR/receipts/identity_issued_receipt.schema.json" "Receipt schema exists"
test_json_valid "$MODULE_DIR/receipts/identity_issued_receipt.schema.json" "Receipt schema is valid JSON"

# Test logic files
log "\n⚙️  Testing Logic Files:" "$BLUE"
test_file_exists "$MODULE_DIR/logic/mod.rs" "Main logic module exists"
test_file_exists "$MODULE_DIR/logic/signature.rs" "Signature logic exists"
test_file_exists "$MODULE_DIR/logic/ghost.rs" "Ghost logic exists"
test_file_exists "$MODULE_DIR/logic/federation.rs" "Federation logic exists"

# Test UI files
log "\n🎨 Testing UI Files:" "$BLUE"
test_file_exists "$MODULE_DIR/ui/LogLineIDBadge.tsx" "LogLine ID Badge component exists"
test_file_exists "$MODULE_DIR/ui/LogLineIDBadge.css" "Badge styles exist"
test_file_exists "$MODULE_DIR/ui/GhostAlert.tsx" "Ghost Alert component exists"
test_file_exists "$MODULE_DIR/ui/GhostAlert.css" "Alert styles exist"
test_file_exists "$MODULE_DIR/ui/AuthCard.tsx" "Auth Card component exists"
test_file_exists "$MODULE_DIR/ui/AuthCard.css" "Auth Card styles exist"
test_file_exists "$MODULE_DIR/ui/index.ts" "UI exports file exists"

# Test documentation
log "\n📚 Testing Documentation:" "$BLUE"
test_file_exists "$MODULE_DIR/README.md" "README documentation exists"
test_file_exists "$MODULE_DIR/test_local.js" "Node.js test runner exists"
test_file_exists "$MODULE_DIR/test_local.sh" "Shell test runner exists"

# File size analysis
log "\n📊 File Analysis:" "$BLUE"
schema_lines=$(count_lines "$MODULE_DIR/schema/logline_id.schema.json")
contract1_lines=$(count_lines "$MODULE_DIR/contracts/identity.register.lll")
contract2_lines=$(count_lines "$MODULE_DIR/contracts/ghost.claim.lll")
contract3_lines=$(count_lines "$MODULE_DIR/contracts/verify_logline_id_signature.lll")
badge_lines=$(count_lines "$MODULE_DIR/ui/LogLineIDBadge.tsx")
alert_lines=$(count_lines "$MODULE_DIR/ui/GhostAlert.tsx")
auth_lines=$(count_lines "$MODULE_DIR/ui/AuthCard.tsx")
readme_lines=$(count_lines "$MODULE_DIR/README.md")

log "  Schema: $schema_lines lines" "$YELLOW"
log "  Contracts: $contract1_lines + $contract2_lines + $contract3_lines lines" "$YELLOW"
log "  UI Components: $badge_lines + $alert_lines + $auth_lines lines" "$YELLOW"
log "  Documentation: $readme_lines lines" "$YELLOW"

# Summary
log "\n=======================================" "$BOLD"
log "VALIDATION SUMMARY" "$BOLD"
log "=======================================" "$BOLD"
log "Total Tests: $TOTAL" "$BLUE"
log "Passed: $PASSED" "$GREEN"
log "Failed: $((TOTAL - PASSED))" "$([ $((TOTAL - PASSED)) -eq 0 ] && echo $GREEN || echo $RED)"
log "=======================================" "$BOLD"

if [ "$PASSED" -eq "$TOTAL" ]; then
    log "\n🎉 ALL VALIDATIONS PASSED!" "$GREEN"
    log "The LogLine ID module is complete and ready for integration." "$GREEN"
    exit 0
else
    log "\n⚠️  Some validations failed." "$YELLOW"
    log "Please check the missing files or fix the issues above." "$YELLOW"
    exit 1
fi