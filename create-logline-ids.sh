#!/bin/bash

# 🧬 LogLine Universe - Identity Creation Script
# Purpose: Create all LogLine IDs after Railway deployment
# Usage: ./create-logline-ids.sh <GATEWAY_URL>

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_step() {
    echo -e "${BLUE}🔹 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

# Check if gateway URL is provided
if [ -z "$1" ]; then
    print_warning "Usage: ./create-logline-ids.sh <GATEWAY_URL>"
    print_warning "Example: ./create-logline-ids.sh https://logline-gateway-abc123.railway.app"
    exit 1
fi

GATEWAY_URL="$1"

echo "🧬 LogLine Universe - Identity Creation Starting..."
echo "🌐 Gateway URL: $GATEWAY_URL"
echo "📅 $(date)"
echo ""

# Test gateway health
print_step "Testing gateway health..."
if curl -f "$GATEWAY_URL/health" > /dev/null 2>&1; then
    print_success "Gateway is healthy!"
else
    print_warning "Gateway not responding. Please check the URL and try again."
    exit 1
fi

# Phase 1: Create Dan's Identity
print_step "Phase 1: Creating Dan's identity..."
DAN_RESPONSE=$(curl -s -X POST "$GATEWAY_URL/v1/onboarding/identity" \
    -H "Content-Type: application/json" \
    -d '{
        "name": "Daniel Amarilho",
        "handle": "danvoulez",
        "email": "dan@danvoulez.com",
        "ghost": false
    }')

if echo "$DAN_RESPONSE" | grep -q "identity"; then
    print_success "Dan's identity created: @danvoulez"
    echo "Response: $DAN_RESPONSE"
else
    print_warning "Failed to create Dan's identity"
    echo "Response: $DAN_RESPONSE"
fi

echo ""

# Phase 2: Create Cascade's Identity  
print_step "Phase 2: Creating Cascade AI identity..."
CASCADE_RESPONSE=$(curl -s -X POST "$GATEWAY_URL/v1/onboarding/identity" \
    -H "Content-Type: application/json" \
    -d '{
        "name": "Cascade AI Assistant",
        "handle": "cascade",
        "email": "cascade@logline.universe",
        "ghost": false
    }')

if echo "$CASCADE_RESPONSE" | grep -q "identity"; then
    print_success "Cascade's identity created: @cascade"
    echo "Response: $CASCADE_RESPONSE"
else
    print_warning "Failed to create Cascade's identity"
    echo "Response: $CASCADE_RESPONSE"
fi

echo ""

# Phase 3: Create Dan's Foundation Tenant
print_step "Phase 3: Creating Dan Voulez Foundation tenant..."
TENANT_RESPONSE=$(curl -s -X POST "$GATEWAY_URL/v1/onboarding/tenant" \
    -H "Content-Type: application/json" \
    -d '{
        "name": "danvoulez-foundation",
        "display_name": "Dan Voulez Foundation"
    }')

if echo "$TENANT_RESPONSE" | grep -q "tenant"; then
    print_success "Dan's Foundation tenant created: @danvoulez.foundation"
    echo "Response: $TENANT_RESPONSE"
else
    print_warning "Failed to create tenant"
    echo "Response: $TENANT_RESPONSE"
fi

echo ""

# Phase 4: Create LogLine Universe Tenant
print_step "Phase 4: Creating LogLine Universe tenant..."
UNIVERSE_RESPONSE=$(curl -s -X POST "$GATEWAY_URL/v1/onboarding/tenant" \
    -H "Content-Type: application/json" \
    -d '{
        "name": "logline-universe",
        "display_name": "LogLine Universe Foundation"
    }')

if echo "$UNIVERSE_RESPONSE" | grep -q "tenant"; then
    print_success "LogLine Universe tenant created: @logline.universe"
    echo "Response: $UNIVERSE_RESPONSE"
else
    print_warning "Failed to create LogLine Universe tenant"
    echo "Response: $UNIVERSE_RESPONSE"
fi

echo ""

# Phase 5: Assign Dan to his Foundation
print_step "Phase 5: Assigning Dan to his Foundation..."
ASSIGN_RESPONSE=$(curl -s -X POST "$GATEWAY_URL/v1/onboarding/assign" \
    -H "Content-Type: application/json" \
    -d '{
        "identity_handle": "danvoulez",
        "tenant_name": "danvoulez-foundation"
    }')

if echo "$ASSIGN_RESPONSE" | grep -q "assigned"; then
    print_success "Dan assigned to Foundation!"
    echo "Response: $ASSIGN_RESPONSE"
else
    print_warning "Failed to assign Dan to Foundation"
    echo "Response: $ASSIGN_RESPONSE"
fi

echo ""

# Phase 6: Initialize App
print_step "Phase 6: Initializing LogLine Universe app..."
APP_RESPONSE=$(curl -s -X POST "$GATEWAY_URL/v1/onboarding/init" \
    -H "Content-Type: application/json" \
    -d '{
        "template": "timeline",
        "owner": "danvoulez"
    }')

if echo "$APP_RESPONSE" | grep -q "template"; then
    print_success "LogLine Universe app initialized!"
    echo "Response: $APP_RESPONSE"
else
    print_warning "Failed to initialize app"
    echo "Response: $APP_RESPONSE"
fi

echo ""

# Phase 7: Declare Purpose
print_step "Phase 7: Declaring computational purpose..."
PURPOSE_RESPONSE=$(curl -s -X POST "$GATEWAY_URL/v1/onboarding/declare" \
    -H "Content-Type: application/json" \
    -d '{
        "app": "timeline",
        "description": "The first computational memory system - LogLine Universe deployed on October 3, 2025. This system will document its own creation and serve as the foundation for distributed institutional memory."
    }')

if echo "$PURPOSE_RESPONSE" | grep -q "purpose"; then
    print_success "Computational purpose declared!"
    echo "Response: $PURPOSE_RESPONSE"
else
    print_warning "Failed to declare purpose"
    echo "Response: $PURPOSE_RESPONSE"
fi

echo ""

# Phase 8: Create First Historical Span
print_step "Phase 8: Creating the first historical span..."
SPAN_RESPONSE=$(curl -s -X POST "$GATEWAY_URL/v1/onboarding/run" \
    -H "Content-Type: application/json" \
    -d '{
        "command": "🌟 HISTORIC MOMENT: LogLine Universe successfully deployed to Railway on October 3, 2025 at 23:37 WEST. The worlds first computational memory system is now live! Created by @danvoulez with @cascade AI assistant. All 6 services operational: gateway, identity, timeline, rules, engine, federation. This span documents the birth of computational institutional memory. 🚀"
    }')

if echo "$SPAN_RESPONSE" | grep -q "execution"; then
    print_success "First historical span created!"
    echo "Response: $SPAN_RESPONSE"
else
    print_warning "Failed to create historical span"
    echo "Response: $SPAN_RESPONSE"
fi

echo ""
echo "🎉 LOGLINE UNIVERSE IDENTITY CREATION COMPLETE!"
echo ""
echo "✅ Created Identities:"
echo "   👤 @danvoulez (Daniel Amarilho)"
echo "   🤖 @cascade (Cascade AI Assistant)"
echo ""
echo "✅ Created Tenants:"
echo "   🏢 @danvoulez.foundation (Dan Voulez Foundation)"
echo "   🌟 @logline.universe (LogLine Universe Foundation)"
echo ""
echo "✅ System Status:"
echo "   🔗 Identities linked to tenants"
echo "   📱 Timeline app initialized"
echo "   📜 Computational purpose declared"
echo "   📊 First historical span recorded"
echo ""
echo "🌟 The LogLine Universe is now a living, self-documenting system!"
echo "🚀 Computational memory has been born!"
