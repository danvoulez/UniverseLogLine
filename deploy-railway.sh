#!/bin/bash

# 🚂 LogLine Universe - Railway Deployment Script
# Created: October 3, 2025
# Purpose: Automated deployment of all LogLine services to Railway

set -e

echo "🚂 LogLine Universe - Railway Deployment Starting..."
echo "📅 $(date)"
echo "🆔 Token: 4bc289e6-a584-4725-970b-b9668b28a26a"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_step() {
    echo -e "${BLUE}🔹 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Check if Railway CLI is installed
if ! command -v railway &> /dev/null; then
    print_warning "Railway CLI not found. Installing..."
    
    # Try different installation methods
    if command -v brew &> /dev/null; then
        print_step "Installing via Homebrew..."
        brew install railway
    elif command -v npm &> /dev/null; then
        print_step "Installing via npm..."
        npm i -g @railway/cli
    else
        print_step "Installing via shell script..."
        bash <(curl -fsSL cli.new)
    fi
    
    print_success "Railway CLI installed!"
else
    print_success "Railway CLI found!"
fi

# Authenticate with Railway
print_step "Authenticating with Railway..."
export RAILWAY_TOKEN="4bc289e6-a584-4725-970b-b9668b28a26a"

# Create new project
print_step "Creating LogLine Universe project..."
railway init --name "logline-universe"

# Link to the project
print_step "Linking to project..."
railway link

# Add PostgreSQL database
print_step "Adding PostgreSQL database..."
railway add --database postgresql

# Add Redis database
print_step "Adding Redis database..."
railway add --database redis

print_success "Databases added!"

# Deploy services one by one
services=("logline-gateway" "logline-id" "logline-timeline" "logline-rules" "logline-engine" "logline-federation")

for service in "${services[@]}"; do
    print_step "Deploying $service..."
    
    # Set environment variables for this service
    case $service in
        "logline-gateway")
            railway variables set SERVICE=logline-gateway
            railway variables set SERVICE_PORT=8080
            railway variables set RUST_LOG=info
            railway variables set GATEWAY_BIND=0.0.0.0:8080
            railway variables set GATEWAY_JWT_SECRET=QQc4wg9wUweVsyWwHUOT5RvXqdz1Q/I8ARJL06ZgfRI=
            railway variables set GATEWAY_JWT_ISSUER="logline-id://tenant/danvoulez.foundation"
            railway variables set GATEWAY_JWT_AUDIENCE=logline-universe
            railway variables set GATEWAY_ALLOWED_ORIGINS="*"
            railway variables set GATEWAY_PUBLIC_PATHS="/healthz,/health,/docs"
            ;;
        "logline-timeline")
            railway variables set SERVICE=logline-timeline
            railway variables set SERVICE_PORT=8080
            railway variables set RUST_LOG=info
            # DATABASE_URL will be automatically set by Railway
            ;;
        "logline-id")
            railway variables set SERVICE=logline-id
            railway variables set SERVICE_PORT=8080
            railway variables set RUST_LOG=info
            ;;
        "logline-rules")
            railway variables set SERVICE=logline-rules
            railway variables set SERVICE_PORT=8080
            railway variables set RUST_LOG=info
            ;;
        "logline-engine")
            railway variables set SERVICE=logline-engine
            railway variables set SERVICE_PORT=8080
            railway variables set RUST_LOG=info
            ;;
        "logline-federation")
            railway variables set SERVICE=logline-federation
            railway variables set SERVICE_PORT=8080
            railway variables set RUST_LOG=info
            railway variables set FEDERATION_NODE_NAME=railway-production
            railway variables set FEDERATION_NETWORK_KEY=logline-universe-alpha
            ;;
    esac
    
    # Deploy the service
    railway up --detach
    
    print_success "$service deployed!"
    sleep 5  # Wait a bit between deployments
done

print_success "All services deployed!"

# Wait for services to be ready
print_step "Waiting for services to be ready..."
sleep 30

# Get service URLs
print_step "Getting service URLs..."
railway status

print_success "🎉 LogLine Universe deployed successfully!"
echo ""
echo "🌟 Next steps:"
echo "1. Check service health at the provided URLs"
echo "2. Create LogLine IDs using the gateway API"
echo "3. Register the first historical spans"
echo ""
echo "🚀 The world's first computational memory system is now live!"
