# LogLine Federation Service

The `logline-federation` service enables distributed operation of LogLine Universe across multiple nodes, providing peer discovery, trust management, and cross-node synchronization capabilities.

## Overview

The federation service transforms LogLine from a single-node system into a distributed network where multiple LogLine instances can:

- **Discover and connect** to peer nodes
- **Establish trust relationships** using cryptographic verification
- **Synchronize timeline data** across the network
- **Maintain data consistency** in a distributed environment
- **Handle network partitions** gracefully

## Architecture

### Core Components

1. **Peer Discovery**: Automatic detection of other LogLine nodes
2. **Trust Management**: Cryptographic verification of node identities
3. **Timeline Synchronization**: Cross-node data replication
4. **Network Communication**: Secure inter-node messaging
5. **Conflict Resolution**: Handling of concurrent updates

### Service Integration

The federation service integrates with other LogLine services:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Gateway       │    │   Timeline      │    │   Identity      │
│   (Auth/Proxy)  │    │   (Data Store)  │    │   (Crypto)      │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
          ┌─────────────────────────────────────────────┐
          │              Federation Service              │
          │         (Cross-Node Coordination)           │
          └─────────────────────┬───────────────────────┘
                                │
    ┌─────────────┬─────────────┼─────────────┬─────────────┐
    │             │             │             │             │
┌───▼───┐    ┌───▼───┐    ┌───▼───┐    ┌───▼───┐    ┌───▼───┐
│Node A │    │Node B │    │Node C │    │Node D │    │Node E │
│(Peer) │    │(Peer) │    │(Peer) │    │(Peer) │    │(Peer) │
└───────┘    └───────┘    └───────┘    └───────┘    └───────┘
```

## API Endpoints

### Health and Status

```http
GET /health
```
Returns the health status of the federation service.

**Response:**
```json
{
  "status": "healthy",
  "node_id": "logline-id://node/local-node",
  "peers_connected": 3,
  "last_sync": "2025-10-03T22:00:00Z"
}
```

### Node Management

```http
GET /v1/nodes
```
Lists all known peer nodes.

**Response:**
```json
{
  "nodes": [
    {
      "id": "logline-id://node/peer-alpha",
      "address": "https://alpha.logline.network",
      "status": "connected",
      "trust_level": "verified",
      "last_seen": "2025-10-03T21:58:30Z"
    }
  ]
}
```

```http
POST /v1/nodes/discover
```
Initiates peer discovery process.

**Request:**
```json
{
  "network_key": "optional-network-identifier",
  "discovery_method": "broadcast|dns|manual"
}
```

### Trust Management

```http
POST /v1/trust/establish
```
Establishes trust relationship with a peer node.

**Request:**
```json
{
  "peer_id": "logline-id://node/peer-beta",
  "peer_address": "https://beta.logline.network",
  "verification_method": "signature|certificate"
}
```

```http
GET /v1/trust/relationships
```
Lists all trust relationships.

**Response:**
```json
{
  "relationships": [
    {
      "peer_id": "logline-id://node/peer-alpha",
      "trust_level": "verified",
      "established_at": "2025-10-03T20:00:00Z",
      "signature": "base64-encoded-signature"
    }
  ]
}
```

### Timeline Synchronization

```http
POST /v1/sync/timeline
```
Synchronizes timeline data with peer nodes.

**Request:**
```json
{
  "peer_ids": ["logline-id://node/peer-alpha"],
  "since": "2025-10-03T20:00:00Z",
  "tenant_filter": "optional-tenant-id"
}
```

```http
GET /v1/sync/status
```
Returns synchronization status.

**Response:**
```json
{
  "last_sync": "2025-10-03T21:55:00Z",
  "pending_syncs": 2,
  "sync_conflicts": 0,
  "peers_in_sync": ["logline-id://node/peer-alpha"]
}
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FEDERATION_NODE_NAME` | Unique name for this node | `local-node` |
| `FEDERATION_NETWORK_KEY` | Network identifier for peer discovery | `` |
| `FEDERATION_TRUSTED_PEERS` | Comma-separated list of trusted peer addresses | `` |
| `FEDERATION_BIND_PORT` | Port for federation service | `8080` |
| `FEDERATION_DISCOVERY_INTERVAL` | Peer discovery interval (seconds) | `300` |
| `FEDERATION_SYNC_INTERVAL` | Timeline sync interval (seconds) | `60` |

### Docker Compose Configuration

```yaml
logline-federation:
  build:
    context: .
    dockerfile: Dockerfile
    args:
      SERVICE: logline-federation
      SERVICE_PORT: 8080
  environment:
    FEDERATION_NODE_NAME: ${FEDERATION_NODE_NAME:-local-node}
    FEDERATION_NETWORK_KEY: ${FEDERATION_NETWORK_KEY:-}
    FEDERATION_TRUSTED_PEERS: ${FEDERATION_TRUSTED_PEERS:-}
  ports:
    - "8084:8080"
```

## Security Model

### Node Authentication

Each node in the federation network must have:

1. **Unique LogLine ID**: Generated by the identity service
2. **Cryptographic Signature**: For message authentication
3. **Trust Verification**: Mutual verification process

### Trust Levels

- **Unknown**: Newly discovered nodes, no trust established
- **Pending**: Trust establishment in progress
- **Verified**: Cryptographically verified and trusted
- **Revoked**: Previously trusted but now untrusted

### Data Protection

- **End-to-End Encryption**: All inter-node communication is encrypted
- **Tenant Isolation**: Federation respects multi-tenant boundaries
- **Access Control**: Only authorized nodes can access specific data

## Federation Patterns

### Peer Discovery

1. **Broadcast Discovery**: Nodes announce themselves on the network
2. **DNS Discovery**: Nodes register with a DNS service
3. **Manual Discovery**: Administrators manually configure peers

### Synchronization Strategies

1. **Event-Driven Sync**: Real-time synchronization on data changes
2. **Periodic Sync**: Regular synchronization intervals
3. **On-Demand Sync**: Manual synchronization requests

### Conflict Resolution

When multiple nodes modify the same data:

1. **Timestamp Ordering**: Later timestamps win
2. **Node Priority**: Designated primary nodes have precedence
3. **Manual Resolution**: Human intervention for complex conflicts

## Integration with Gateway

The federation service integrates with the LogLine Gateway:

```yaml
# Gateway configuration
FEDERATION_URL: http://logline-federation:8080
```

### Authentication Flow

1. **Client Request**: Client makes request to gateway
2. **JWT Validation**: Gateway validates client JWT token
3. **Federation Check**: Gateway checks if request requires federation
4. **Cross-Node Call**: Gateway forwards request to federation service
5. **Peer Communication**: Federation service communicates with peers
6. **Response Aggregation**: Results are aggregated and returned

## Monitoring and Observability

### Health Checks

The federation service provides comprehensive health monitoring:

- **Node Connectivity**: Status of connections to peer nodes
- **Sync Status**: Timeline synchronization health
- **Trust Status**: Validity of trust relationships
- **Network Latency**: Performance metrics for peer communication

### Logging

Federation events are logged with structured logging:

```json
{
  "timestamp": "2025-10-03T22:00:00Z",
  "level": "INFO",
  "event": "peer_connected",
  "peer_id": "logline-id://node/peer-alpha",
  "latency_ms": 45
}
```

### Metrics

Key metrics exposed for monitoring:

- `federation_peers_connected`: Number of connected peers
- `federation_sync_duration_seconds`: Timeline sync duration
- `federation_trust_relationships_total`: Total trust relationships
- `federation_network_latency_seconds`: Network latency to peers

## Deployment Considerations

### Network Requirements

- **Connectivity**: Nodes must be able to reach each other
- **Ports**: Federation port (8084) must be accessible
- **Firewall**: Configure firewall rules for peer communication

### Scalability

- **Horizontal Scaling**: Add more nodes to increase capacity
- **Geographic Distribution**: Deploy nodes across regions
- **Load Balancing**: Distribute requests across peer nodes

### Backup and Recovery

- **Data Replication**: Timeline data is replicated across nodes
- **Node Recovery**: Failed nodes can rejoin and resync
- **Disaster Recovery**: Network can survive partial node failures

## CLI Integration

The federation service integrates with the LogLine CLI:

```bash
# Discover peers
logline federation discover

# Establish trust with a peer
logline federation trust add --peer "https://peer.logline.network"

# Sync timeline data
logline federation sync --since "1 hour ago"

# Show federation status
logline federation status
```

## Future Enhancements

### Planned Features

1. **Smart Routing**: Intelligent request routing based on data locality
2. **Consensus Protocols**: Implement Raft or similar for strong consistency
3. **Edge Caching**: Cache frequently accessed data at edge nodes
4. **Cross-Tenant Federation**: Controlled sharing between different tenants
5. **Blockchain Integration**: Optional blockchain backend for immutable logs

### Performance Optimizations

1. **Delta Synchronization**: Only sync changed data
2. **Compression**: Compress inter-node communication
3. **Connection Pooling**: Reuse connections to peer nodes
4. **Batch Operations**: Batch multiple operations for efficiency

## Troubleshooting

### Common Issues

**Peer Discovery Fails**
- Check network connectivity
- Verify FEDERATION_NETWORK_KEY configuration
- Ensure firewall allows federation port

**Trust Establishment Fails**
- Verify peer LogLine ID is valid
- Check cryptographic signatures
- Ensure peer is reachable

**Sync Conflicts**
- Review conflict resolution logs
- Check timestamp synchronization
- Verify data integrity

### Debug Commands

```bash
# Check federation service logs
docker logs logline-federation

# Test peer connectivity
curl http://localhost:8084/v1/nodes

# Verify trust relationships
curl http://localhost:8084/v1/trust/relationships
```

---

The LogLine Federation Service enables the LogLine Universe to operate as a truly distributed system, providing the foundation for scalable, resilient, and globally distributed logging infrastructure.
