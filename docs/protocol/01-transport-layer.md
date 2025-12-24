# Transport Layer

The lowest layer of the VSCode Remote protocol stack.

## Stack Overview

```
Application (IPC Channels)
    ↓
PersistentProtocol (Message framing, ACKs, reconnection)
    ↓
WebSocket (Framing, optional compression)
    ↓
TCP (over SSH tunnel)
    ↓
SSH (Encryption, authentication)
    ↓
Network
```

## PersistentProtocol

**Location**: `uplink-server/vscode-server/src/vs/base/parts/ipc/common/ipc.net.ts`

### Message Format

Every message has a 13-byte header:

```
┌──────┬──────┬──────┬───────────────┬──────────────┐
│ TYPE │  ID  │ ACK  │ DATA_LENGTH   │   PAYLOAD    │
│  1B  │  4B  │  4B  │     4B        │   variable   │
└──────┴──────┴──────┴───────────────┴──────────────┘
```

**Header Fields**:
- `TYPE` (1 byte): Message type (see below)
- `ID` (4 bytes, u32be): Message sequence number
- `ACK` (4 bytes, u32be): Acknowledged message ID
- `DATA_LENGTH` (4 bytes, u32be): Payload size

### Message Types

```typescript
enum ProtocolMessageType {
  None = 0,           // No-op
  Regular = 1,        // Data message (counted & acked)
  Control = 2,        // Control message (not counted)
  Ack = 3,            // Acknowledgment only
  Disconnect = 5,     // Graceful disconnect
  ReplayRequest = 6,  // Request message replay
  Pause = 7,          // Pause sending
  Resume = 8,         // Resume sending
  KeepAlive = 9       // Keep connection alive
}
```

**Implementation**: `class PersistentProtocol` in `ipc.net.ts`

### Protocol Constants

```typescript
enum ProtocolConstants {
  HeaderLength = 13,
  AcknowledgeTime = 2000,              // ACK within 2s
  TimeoutTime = 20000,                 // Timeout after 20s
  ReconnectionGraceTime = 10800000,    // 3 hours
  ReconnectionShortGraceTime = 300000, // 5 minutes
  KeepAliveSendTime = 5000             // Every 5s
}
```

### Reliability Features

**Acknowledgments**:
- Every Regular message gets an ID
- Receiver must ACK within 2 seconds
- Sender buffers unacknowledged messages
- Timeout if no ACK after 20 seconds

**Reconnection**:
- Client reconnects with same `reconnectionToken`
- Server maintains state for 3 hours
- Buffered messages replayed on reconnection
- No message loss

**Flow Control**:
- Pause/Resume messages
- Prevents buffer overflow
- Backpressure handling

## WebSocket Layer

**Location**: `uplink-server/vscode-server/src/vs/base/parts/ipc/node/ipc.net.ts`

### Upgrade Process

```typescript
// Server: remoteExtensionHostAgentServer.ts
server.on('upgrade', async (req, socket) => {
  const upgraded = upgradeToISocket(req, socket, {
    debugLabel: `server-connection-${reconnectionToken}`,
    skipWebSocketFrames: false,
    disableWebSocketCompression: false
  });
  
  this._handleWebSocketConnection(upgraded, isReconnection, reconnectionToken);
});
```

### WebSocket Features

- **Framing**: Standard WebSocket frames
- **Compression**: Optional gzip (can be disabled)
- **Binary mode**: Opcode 2 (binary frames)
- **Masking**: Client-to-server frames masked

## SSH Tunnel

**Location**: `uplink-vscode-extension/src/authResolver.ts`

### Tunnel Setup

```typescript
// Extension creates SSH tunnel
const tunnel = await this.openTunnel(0, serverPort);

// Returns local port forwarding
return new ResolvedAuthority(
  '127.0.0.1',
  tunnel.localPort,
  connectionToken
);
```

### Native SSH Implementation

**Location**: `uplink-vscode-extension/native/src/lib.rs`

```rust
// Rust SSH client using russh
pub fn ssh_forward_port(
    session_id: i32,
    local_port: u16,
    remote_host: String,
    remote_port: u16,
) -> Result<u16> {
    // Creates TCP listener on local_port
    // Forwards connections to remote_host:remote_port
    // Returns actual local port bound
}
```

**Features**:
- Certificate authentication (ecdsa-sha2-nistp256-cert-v01@openssh.com)
- Direct TCP forwarding (no SOCKS)
- Session management
- Automatic reconnection

## Connection Handshake

**Location**: `uplink-server/vscode-server/src/vs/server/node/remoteExtensionHostAgentServer.ts`

### Handshake Sequence

```
Client                           Server
──────────────────────────────────────────

1. WebSocket Upgrade
   GET /?reconnectionToken=<uuid>
   Upgrade: websocket
                                 ↓
                            2. Accept upgrade
                               ↓
                            3. Wait for auth
   ←─────────────────────  
4. Send auth
   { type: "auth", 
     auth: "<token>",
     data: "<uuid>" }
                                 ↓
                            5. Validate token
                               ↓
                            6. Send challenge
   ←─────────────────────  { type: "sign",
                              data: "<challenge>",
                              signedData: "<sig>" }
7. Send connection type
   { type: "connectionType",
     commit: "<hash>",
     signedData: "<response>",
     desiredConnectionType: 1 }
                                 ↓
                            8. Validate commit
                               ↓
                            9. Send OK
   ←─────────────────────  { type: "ok" }

10. PersistentProtocol active
```

**Implementation**: `_handleWebSocketConnection()` method

### Connection Types

```typescript
enum ConnectionType {
  Management = 1,      // Main control connection
  ExtensionHost = 2,   // Extension host process
  Tunnel = 3           // Port forwarding
}
```

## Performance Characteristics

### Latency
- **WebSocket overhead**: ~1-2ms
- **Protocol framing**: ~1ms
- **SSH encryption**: ~1-2ms
- **Total overhead**: ~3-5ms per message

### Throughput
- **Raw TCP**: ~1 Gbps (local)
- **With SSH**: ~500 Mbps
- **With WebSocket**: ~400 Mbps
- **With Protocol**: ~300 Mbps

### Optimization Opportunities

1. **Custom compression**: Replace gzip with zstd/lz4
2. **Binary protocol**: Already binary, but could optimize serialization
3. **Multiplexing**: Already supported via message IDs
4. **QUIC transport**: Replace TCP with QUIC for better performance
5. **Delta encoding**: Only send file changes

## Code References

### Extension
- `src/authResolver.ts:openTunnel()` - Tunnel creation
- `src/nativeSSHConnection.ts:forwardPort()` - Port forwarding
- `native/src/lib.rs:ssh_forward_port()` - Rust SSH implementation

### Server
- `vscode-server/src/vs/base/parts/ipc/common/ipc.net.ts` - PersistentProtocol
- `vscode-server/src/vs/base/parts/ipc/node/ipc.net.ts` - WebSocket handling
- `vscode-server/src/vs/server/node/remoteExtensionHostAgentServer.ts` - Handshake
- `vscode-server/src/server-main.ts` - HTTP server setup
