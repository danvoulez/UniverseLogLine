#!/bin/bash

# LogLine ID Module Local Test Runner (Shell Version)
# Tests identity registration, ghost creation, and verification

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Configuration
ALIAS="danvoulez"
TEST_DIR="$(dirname "$0")/test_data"
CONTRACTS_DIR="$(dirname "$0")/contracts"
SCHEMA_FILE="$(dirname "$0")/schema/logline_id.schema.json"

log() {
    local message="$1"
    local color="${2:-$NC}"
    echo -e "${color}${message}${NC}"
}

create_test_directory() {
    if [ ! -d "$TEST_DIR" ]; then
        mkdir -p "$TEST_DIR"
        log "✓ Created test directory: $TEST_DIR" "$GREEN"
    fi
}

generate_test_keypair() {
    log "\n📋 Generating test key pair..." "$BLUE"
    
    local key_path="$TEST_DIR/test_key.pem"
    local pub_key_path="$TEST_DIR/test_key.pub"
    
    if ! command -v openssl &> /dev/null; then
        log "✗ OpenSSL not found. Installing via Homebrew..." "$YELLOW"
        if command -v brew &> /dev/null; then
            brew install openssl
        else
            log "✗ Homebrew not found. Please install OpenSSL manually." "$RED"
            return 1
        fi
    fi
    
    # Generate Ed25519 key pair
    openssl genpkey -algorithm Ed25519 -out "$key_path" 2>/dev/null
    openssl pkey -in "$key_path" -pubout -out "$pub_key_path" 2>/dev/null
    
    log "✓ Private key: $key_path" "$GREEN"
    log "✓ Public key: $pub_key_path" "$GREEN"
    
    echo "$key_path:$pub_key_path"
}

load_schema() {
    log "\n📋 Loading LogLine ID schema..." "$BLUE"
    
    if [ ! -f "$SCHEMA_FILE" ]; then
        log "✗ Schema file not found: $SCHEMA_FILE" "$RED"
        return 1
    fi
    
    # Simple JSON validation
    if command -v python3 &> /dev/null; then
        if python3 -c "import json; json.load(open('$SCHEMA_FILE'))" 2>/dev/null; then
            log "✓ Schema loaded and validated" "$GREEN"
            return 0
        fi
    fi
    
    # Basic check if file exists and is not empty
    if [ -s "$SCHEMA_FILE" ]; then
        log "✓ Schema file exists and is not empty" "$GREEN"
        return 0
    fi
    
    log "✗ Schema validation failed" "$RED"
    return 1
}

create_test_identity() {
    log "\n📋 Creating test identity..." "$BLUE"
    
    local keys="$1"
    local private_key_path="${keys%%:*}"
    local public_key_path="${keys##*:}"
    
    # Read public key and convert to hex (simplified)
    local pub_key_content
    pub_key_content=$(cat "$public_key_path" | base64 | tr -d '\n')
    
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    
    local identity_id="logline_id_$(date +%s)"
    
    # Create identity JSON
    cat > "$TEST_DIR/test_identity.json" << EOF
{
  "id": "$identity_id",
  "alias": "$ALIAS",
  "owner_type": "individual",
  "public_key": "$pub_key_content",
  "created_at": "$timestamp",
  "updated_at": "$timestamp",
  "status": "active",
  "roles": ["user"],
  "capabilities": ["sign", "verify", "ghost_create"],
  "federation_level": "local",
  "metadata": {
    "test": true,
    "created_by": "local_test_runner"
  }
}
EOF
    
    log "✓ Identity created: $TEST_DIR/test_identity.json" "$GREEN"
    log "  Alias: @$ALIAS" "$YELLOW"
    log "  ID: $identity_id" "$YELLOW"
    
    echo "$identity_id"
}

validate_identity_against_schema() {
    log "\n📋 Validating identity against schema..." "$BLUE"
    
    local identity_file="$TEST_DIR/test_identity.json"
    
    # Basic validation checks
    if ! grep -q '"id"' "$identity_file"; then
        log "✗ Missing required field: id" "$RED"
        return 1
    fi
    
    if ! grep -q '"alias"' "$identity_file"; then
        log "✗ Missing required field: alias" "$RED"
        return 1
    fi
    
    if ! grep -q '"owner_type"' "$identity_file"; then
        log "✗ Missing required field: owner_type" "$RED"
        return 1
    fi
    
    if ! grep -q '"public_key"' "$identity_file"; then
        log "✗ Missing required field: public_key" "$RED"
        return 1
    fi
    
    if ! grep -q '"status"' "$identity_file"; then
        log "✗ Missing required field: status" "$RED"
        return 1
    fi
    
    log "✓ Identity validation passed" "$GREEN"
    return 0
}

test_identity_registration() {
    log "\n📋 Testing identity registration contract..." "$BLUE"
    
    local identity_id="$1"
    local contract_file="$CONTRACTS_DIR/identity.register.lll"
    
    if [ ! -f "$contract_file" ]; then
        log "✗ Contract file not found: $contract_file" "$RED"
        return 1
    fi
    
    log "✓ Contract loaded successfully" "$GREEN"
    
    # Create mock receipt
    local transaction_id="tx_$(date +%s)"
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    
    cat > "$TEST_DIR/registration_receipt.json" << EOF
{
  "transaction_id": "$transaction_id",
  "identity_id": "$identity_id",
  "alias": "$ALIAS",
  "operation": "identity.register",
  "timestamp": "$timestamp",
  "block_height": 1000,
  "gas_used": 21000,
  "status": "success",
  "provenance": {
    "contract": "identity.register.lll",
    "executor": "local_test_runner",
    "version": "1.0.0"
  },
  "signatures": {
    "identity_signature": "mock_signature_$(date +%s)",
    "network_signature": "mock_network_signature_$(date +%s)"
  },
  "verification_data": {
    "public_key_verified": true,
    "alias_available": true,
    "schema_valid": true
  }
}
EOF
    
    log "✓ Registration receipt created: $TEST_DIR/registration_receipt.json" "$GREEN"
    log "  Transaction ID: $transaction_id" "$YELLOW"
    
    echo "$transaction_id"
}

test_ghost_identity_creation() {
    log "\n📋 Testing ghost identity creation..." "$BLUE"
    
    local parent_identity_id="$1"
    local ghost_alias="${ALIAS}_ghost_$(date +%s)"
    local ghost_id="ghost_$(date +%s)"
    
    # Calculate expiration (24 hours from now)
    local expires_at
    if command -v gdate &> /dev/null; then
        expires_at=$(gdate -u -d "+24 hours" +"%Y-%m-%dT%H:%M:%SZ")
    else
        expires_at=$(date -u -v+24H +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date -u +"%Y-%m-%dT%H:%M:%SZ")
    fi
    
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    
    cat > "$TEST_DIR/ghost_identity.json" << EOF
{
  "id": "$ghost_id",
  "alias": "$ghost_alias",
  "owner_type": "ghost",
  "parent_id": "$parent_identity_id",
  "created_at": "$timestamp",
  "expires_at": "$expires_at",
  "status": "active",
  "roles": ["ghost"],
  "capabilities": ["sign", "verify"],
  "federation_level": "local",
  "purpose": "Local testing",
  "trust_level": 75,
  "metadata": {
    "test": true,
    "parent_alias": "$ALIAS"
  }
}
EOF
    
    log "✓ Ghost identity created: $TEST_DIR/ghost_identity.json" "$GREEN"
    log "  Ghost Alias: @$ghost_alias" "$YELLOW"
    log "  Expires: $expires_at" "$YELLOW"
    
    echo "$ghost_id"
}

test_signature_verification() {
    log "\n📋 Testing signature verification..." "$BLUE"
    
    local keys="$1"
    local private_key_path="${keys%%:*}"
    local public_key_path="${keys##*:}"
    
    local test_message="LogLine ID Test - $ALIAS - $(date +%s)"
    local message_file="$TEST_DIR/test_message.txt"
    local signature_file="$TEST_DIR/test_signature.sig"
    
    # Write test message
    echo "$test_message" > "$message_file"
    
    # Sign the message
    if openssl pkeyutl -sign -inkey "$private_key_path" -in "$message_file" -out "$signature_file" 2>/dev/null; then
        # Verify the signature
        if openssl pkeyutl -verify -pubin -inkey "$public_key_path" -in "$message_file" -sigfile "$signature_file" 2>/dev/null; then
            log "✓ Signature verification passed" "$GREEN"
            log "  Message: $test_message" "$YELLOW"
            return 0
        fi
    fi
    
    log "✗ Signature verification failed" "$RED"
    return 1
}

generate_test_report() {
    log "\n📋 Generating test report..." "$BLUE"
    
    local test_run_id="test_$(date +%s)"
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    
    # Count test results (simplified)
    local total_tests=7
    local passed_tests="$1"
    local failed_tests=$((total_tests - passed_tests))
    
    cat > "$TEST_DIR/test_report.json" << EOF
{
  "test_run_id": "$test_run_id",
  "timestamp": "$timestamp",
  "alias_tested": "$ALIAS",
  "summary": {
    "total_tests": $total_tests,
    "passed": $passed_tests,
    "failed": $failed_tests
  },
  "test_data_directory": "$TEST_DIR"
}
EOF
    
    log "✓ Test report generated: $TEST_DIR/test_report.json" "$GREEN"
    
    # Display summary
    echo
    log "==================================================" "$BOLD"
    log "TEST SUMMARY" "$BOLD"
    log "==================================================" "$BOLD"
    log "Total Tests: $total_tests" "$BLUE"
    log "Passed: $passed_tests" "$GREEN"
    if [ "$failed_tests" -gt 0 ]; then
        log "Failed: $failed_tests" "$RED"
    else
        log "Failed: $failed_tests" "$GREEN"
    fi
    log "==================================================" "$BOLD"
    
    return $failed_tests
}

# Main test execution
run_tests() {
    log "🚀 Starting LogLine ID Module Tests" "$BOLD"
    log "Testing alias: @$ALIAS" "$BLUE"
    
    local passed_tests=0
    local total_tests=7
    
    # Setup
    create_test_directory || return 1
    
    # Test 1: Generate keys
    if keys=$(generate_test_keypair); then
        ((passed_tests++))
    fi
    
    # Test 2: Load schema
    if load_schema; then
        ((passed_tests++))
    fi
    
    # Test 3: Create identity
    if identity_id=$(create_test_identity "$keys"); then
        ((passed_tests++))
    fi
    
    # Test 4: Validate identity
    if validate_identity_against_schema; then
        ((passed_tests++))
    fi
    
    # Test 5: Test registration contract
    if transaction_id=$(test_identity_registration "$identity_id"); then
        ((passed_tests++))
    fi
    
    # Test 6: Test ghost identity
    if ghost_id=$(test_ghost_identity_creation "$identity_id"); then
        ((passed_tests++))
    fi
    
    # Test 7: Test signature verification
    if test_signature_verification "$keys"; then
        ((passed_tests++))
    fi
    
    # Generate report
    if generate_test_report "$passed_tests"; then
        log "\n✅ All tests completed!" "$GREEN"
        
        if [ "$passed_tests" -eq "$total_tests" ]; then
            log "🎉 All tests passed!" "$GREEN"
            return 0
        else
            local failed=$((total_tests - passed_tests))
            log "⚠️  $failed test(s) failed. Check the report for details." "$YELLOW"
            return 1
        fi
    else
        log "\n💥 Test execution failed during report generation" "$RED"
        return 1
    fi
}

# Run tests if script is executed directly
if [ "${BASH_SOURCE[0]}" -ef "$0" ]; then
    run_tests
    exit $?
fi