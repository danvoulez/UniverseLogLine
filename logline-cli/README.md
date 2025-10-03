# LogLine CLI

The official command-line interface for the LogLine Universe - a distributed logging and identity system.

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/danvoulez/UniverseLogLine.git
cd UniverseLogLine

# Build the CLI
cargo build --release -p logline-cli

# The binary will be available at target/release/logline
```

### Using Cargo

```bash
cargo install --path logline-cli
```

## Quick Start

### 1. Create Your Identity

```bash
# Create a new LogLine ID identity
logline create identity --name "Your Name" --handle yourhandle
```

### 2. Create or Join a Tenant

```bash
# Create a new organization/tenant
logline create tenant --name "Your Organization"

# Or assign yourself to an existing tenant
logline assign identity yourhandle --to tenant existing-org
```

### 3. Initialize an Application

```bash
# Initialize with a template
logline init app --template minicontratos --owner yourhandle
```

### 4. Declare Your Purpose

```bash
# Declare what you want to accomplish
logline declare purpose --app minicontratos --description "Track team activities and contracts"
```

### 5. Start Using the System

```bash
# Run the interactive shell
logline run shell

# Or execute specific commands
logline run command "register payment €100 to John, completed yesterday"
```

## Commands

### Identity Management

```bash
# Create a new identity
logline create identity --name "Alice Smith" --handle alice

# Generate a standalone LogLine ID
logline generate-id --node-name my-laptop
```

### Tenant Management

```bash
# Create a new tenant/organization
logline create tenant --name "Acme Corp" --display-name "Acme Corporation"

# Assign identity to tenant
logline assign identity alice --to tenant acme-corp
```

### Application Management

```bash
# Initialize application with template
logline init app --template minicontratos --owner alice

# Available templates:
# - minicontratos: Contract and task management
# - timeline: Activity timeline tracking
# - vtv: Video content management
# - ghost: Anonymous/ephemeral mode
```

### Purpose Declaration

```bash
# Declare computational purpose
logline declare purpose --app minicontratos \
  --description "Manage contracts and track deliverables for Acme Corp"
```

### Execution

```bash
# Interactive shell
logline run shell

# Direct command execution
logline run command "create task: Review Q3 budget, due next Friday"
```

### Utility Commands

```bash
# Show version
logline version

# Show help
logline --help
logline create --help
```

## Configuration

The CLI can be configured through:

1. **Environment Variables**
2. **Command-line flags**
3. **Configuration file** (coming soon)

### Environment Variables

```bash
# Gateway URL (required)
export LOGLINE_GATEWAY_URL="http://localhost:8070"

# Optional: Custom config directory
export LOGLINE_CONFIG_DIR="$HOME/.config/logline"
```

### Command-line Flags

```bash
# Specify gateway URL
logline --gateway http://localhost:8070 create identity --name "Alice"

# All commands support the --gateway flag
```

## Authentication

The CLI automatically handles authentication:

1. **First Use**: Creates local session after identity creation
2. **Subsequent Uses**: Uses stored JWT tokens
3. **Token Refresh**: Automatically refreshes expired tokens
4. **Multi-tenant**: Supports switching between tenants

### Session Management

Sessions are stored in:
- **macOS**: `~/Library/Application Support/logline/`
- **Linux**: `~/.config/logline/`
- **Windows**: `%APPDATA%\logline\`

```bash
# Session files:
# - session.json: Current session and JWT tokens
# - identities.json: Known identities and their keys
```

## Examples

### Complete Onboarding Flow

```bash
# 1. Create identity
logline create identity --name "Alice Smith" --handle alice

# 2. Create organization
logline create tenant --name "acme-corp" --display-name "Acme Corporation"

# 3. Assign identity to organization
logline assign identity alice --to tenant acme-corp

# 4. Initialize application
logline init app --template minicontratos --owner alice

# 5. Declare purpose
logline declare purpose --app minicontratos \
  --description "Track contracts and deliverables for Acme Corp"

# 6. Start using the system
logline run shell
```

### Working with Multiple Tenants

```bash
# Create personal tenant
logline create tenant --name "alice-personal"

# Create work tenant
logline create tenant --name "acme-corp"

# Switch between tenants (context switching)
logline assign identity alice --to tenant alice-personal
logline run command "personal task: Buy groceries"

logline assign identity alice --to tenant acme-corp
logline run command "work task: Prepare presentation"
```

### Federation Commands

```bash
# Discover peer nodes
logline federation discover

# Establish trust with a peer
logline federation trust add --peer "https://peer.logline.network"

# Sync timeline data
logline federation sync --since "1 hour ago"

# Show federation status
logline federation status
```

## Development

### Building

```bash
# Build in development mode
cargo build -p logline-cli

# Build optimized release
cargo build --release -p logline-cli

# Run tests
cargo test -p logline-cli
```

### Testing

```bash
# Unit tests
cargo test -p logline-cli

# Integration tests (requires running gateway)
LOGLINE_GATEWAY_URL=http://localhost:8070 cargo test -p logline-cli --test integration
```

## Troubleshooting

### Common Issues

**"Gateway not reachable"**
```bash
# Check if gateway is running
curl http://localhost:8070/health

# Verify gateway URL
echo $LOGLINE_GATEWAY_URL
```

**"Authentication failed"**
```bash
# Check session status
ls ~/.config/logline/

# Clear session and re-authenticate
rm ~/.config/logline/session.json
logline create identity --name "Your Name" --handle yourhandle
```

**"Identity not found"**
```bash
# List known identities
cat ~/.config/logline/identities.json

# Create new identity if needed
logline create identity --name "Your Name" --handle yourhandle
```

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=debug logline create identity --name "Alice"

# Trace all HTTP requests
RUST_LOG=trace logline run command "test command"
```

## Architecture

The CLI is structured as:

```
logline-cli/
├── src/
│   ├── main.rs           # CLI entry point and command parsing
│   ├── onboarding.rs     # Onboarding flow implementation
│   ├── client.rs         # HTTP client for gateway communication
│   ├── session.rs        # Session and authentication management
│   ├── config.rs         # Configuration management
│   └── utils.rs          # Utility functions
├── tests/
│   ├── integration.rs    # Integration tests
│   └── fixtures/         # Test fixtures
└── Cargo.toml           # Dependencies and metadata
```

### Key Components

1. **Command Parser**: Uses `clap` for robust CLI parsing
2. **HTTP Client**: `reqwest` for gateway communication
3. **Session Management**: Secure token storage and refresh
4. **Onboarding Flow**: Step-by-step user onboarding
5. **Error Handling**: Comprehensive error messages

## Contributing

1. **Fork the repository**
2. **Create a feature branch**: `git checkout -b feature/amazing-feature`
3. **Make your changes**
4. **Add tests**: Ensure your changes are tested
5. **Run tests**: `cargo test -p logline-cli`
6. **Commit**: `git commit -m 'Add amazing feature'`
7. **Push**: `git push origin feature/amazing-feature`
8. **Create Pull Request**

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

## Support

- **Documentation**: [docs/](../docs/)
- **Issues**: [GitHub Issues](https://github.com/danvoulez/UniverseLogLine/issues)
- **Discussions**: [GitHub Discussions](https://github.com/danvoulez/UniverseLogLine/discussions)
