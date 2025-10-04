# LogLine Deployment Cleanup Protocol

## Post-Deployment Sanitization

### 1. Temporary File Inventory

Locations to clean:

```bash
# Diagnostic artifacts
/tmp/logline-*.log

# Build artifacts
/Users/voulezvous/Downloads/logline_id_unificado/target/
~/.cargo/registry/

# Old configurations
~/.config/logline/session.toml.old
/usr/local/etc/logline/config.toml.bak
```

### 2. Automated Cleanup Script

```bash
#!/usr/bin/env bash
# save as: scripts/cleanup-logline.sh

echo "🌀 LogLine Cleanup Utility"

# 1. Log Files
echo "🧹 Cleaning log files..."
find /tmp -name "logline-*.log" -mtime +7 -delete

# 2. Build Artifacts
echo "🛠️  Cleaning build artifacts..."
(cd /Users/voulezvous/Downloads/logline_id_unificado && cargo clean)

# 3. Configuration Versions
echo "⚙️  Pruning old configs:"
find ~/.config/logline/ -name "*.old" -delete
find /usr/local/etc/logline/ -name "*.bak" -delete

# 4. Service Reset
echo "♻️  Resetting service:"
sudo launchctl unload /Library/LaunchDaemons/com.danvoulez.logline.plist 2>/dev/null
sudo rm -f /Library/LaunchDaemons/com.danvoulez.logline.plist

echo "✅ Cleanup complete"
```

### 3. Scheduled Maintenance

Add to crontab (runs weekly):

```bash
(crontab -l 2>/dev/null; echo "0 3 * * 0 /Users/voulezvous/Downloads/logline_id_unificado/scripts/cleanup-logline.sh") | crontab -
```

### 4. Verification Checks

```bash
# Post-cleanup validation
du -sh /tmp/logline-*  # Should show 0B
cargo metadata --format-version 1 | jq '.target_directory' # Verify clean build
```

### 5. Safe File Retention

Preserve these critical files:

- /usr/local/bin/logline
- /usr/local/etc/logline/config.toml
- /Library/LaunchDaemons/com.danvoulez.logline.plist
- ~/.config/logline/session.toml

## Cleanup Safety Protocol

### Dry Run First

```bash
./scripts/cleanup-logline.sh --dry-run
```

### Confirmation Prompt

Add to script:

```bash
read -p "Delete 7+ day old logs? (y/n): " confirm
[[ $confirm == [yY] ]] || exit
```

## Post-Cleanup Checklist

Verify service restarts:

```bash
sudo launchctl load /Library/LaunchDaemons/com.danvoulez.logline.plist
```

Confirm API availability:

```bash
curl -sI http://100.102.181.95:8070/health | head -1
```

Check Tailscale peers:

```bash
tailscale status | grep -E "(active)"
```
