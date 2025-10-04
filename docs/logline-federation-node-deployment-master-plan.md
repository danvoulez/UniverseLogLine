# LogLine Federation Node Deployment Master Plan

## Current Obstacles

### Code Issues
- Missing critical Rust imports (Arc, Router, etc.)
- Incomplete error handling
- No explicit Tailscale IP binding

### Service Management
- Launchd configuration errors
- No persistence across reboots
- Missing log rotation

### Network Configuration
- Port 8070 not properly exposed
- Firewall restrictions
- Tailscale peer verification needed

## Required Phases

### 1. Codebase Remediation
```rust
// src/main.rs critical fixes:
use std::sync::Arc;
use axum::{Router, Extension};
use axum::routing::{get, post};
use tower_http::trace::TraceLayer;

// Add to Cargo.toml:
[dependencies]
tower-http = { version = "0.5", features = ["trace"] }
```

### 2. Build & Deployment
```bash
# Clean rebuild
cargo build --release --bin logline

# Install
sudo install -m 0755 target/release/logline /usr/local/bin/

# Configuration
mkdir -p /usr/local/etc/logline
cat > /usr/local/etc/logline/config.toml <<'CONFIG'
[network]
tailscale_ip = "100.102.181.95"
federation_peers = ["100.72.6.121"]
CONFIG
```

### 3. Service Orchestration
```bash
# Systemd-style service (macOS launchd alternative)
sudo tee /Library/LaunchDaemons/com.danvoulez.logline.plist <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.danvoulez.logline</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/logline</string>
        <string>--config</string>
        <string>/usr/local/etc/logline/config.toml</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
PLIST

# Activate
sudo launchctl bootstrap system /Library/LaunchDaemons/com.danvoulez.logline.plist
```

### 4. Network Finalization
```bash
# Tailscale
sudo tailscale up --advertise-exit-node --accept-routes

# Firewall
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /usr/local/bin/logline
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblock /usr/local/bin/logline
```

### 5. Verification Matrix

| Check | Command | Expected Output |
| --- | --- | --- |
| Service Status | `sudo launchctl list | grep logline` | Process entry present |
| API Health | `curl -s http://100.102.181.95:8070/health` | HTTP 200 OK |
| Tailscale | `tailscale status | grep macmini` | `100.102.181.95 active` |
| Logs | `tail /usr/local/var/log/logline.log` | No `ERROR` entries |

## Timeline

### Immediate (5 mins)
- Fix code imports
- Manual test run

### Phase 1 (15 mins)
- Implement config file
- Basic service install

### Phase 2 (30 mins)
- Full persistence setup
- Network hardening

### Phase 3 (Ongoing)
- Monitoring integration
- Peer synchronization

## Risk Mitigation

### Fallback Plan
```bash
# Emergency start script
nohup /usr/local/bin/logline run --gateway http://100.102.181.95:8070 > /tmp/logline-fallback.log 2>&1 &
```

Would you like me to proceed with any specific phase or modify the approach? This provides the complete roadmap to production-grade deployment.
