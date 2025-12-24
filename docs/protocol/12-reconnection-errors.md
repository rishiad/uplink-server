# Reconnection Protocol & Error Handling

How VSCode Remote handles disconnections, reconnections, and errors.

## Reconnection Architecture

```
┌─────────────────────────────────────────────┐
│ PersistentProtocol                          │
│  ├─ Message queue (unacknowledged)          │
│  ├─ ACK tracking                            │
│  ├─ Reconnection token                      │
│  └─ Grace period timer                      │
└─────────────────────────────────────────────┘
```

## Protocol Constants

```typescript
enum ProtocolConstants {
  HeaderLength = 13,              // 13-byte header
  AcknowledgeTime = 2000,         // Send ACK within 2s
  TimeoutTime = 20000,            // 20s timeout
  ReconnectionGraceTime = 10800000,  // 3 hours
  ReconnectionShortGraceTime = 300000,  // 5 minutes
  KeepAliveSendTime = 5000        // 5s keep-alive
}
```

## Message Acknowledgment

### ACK Flow

```
Client                              Server
   │                                   │
   │──── Msg(id=1, ack=0) ────────────▶│
   │                                   │
   │◀──── Msg(id=1, ack=1) ────────────│  (ACKs client msg 1)
   │                                   │
   │──── Msg(id=2, ack=1) ────────────▶│  (ACKs server msg 1)
   │                                   │
   │◀──── ACK(ack=2) ─────────────────│  (Pure ACK)
```

### Unacknowledged Queue

```typescript
class PersistentProtocol {
  private _outgoingUnackMsg: Queue<ProtocolMessage>;
  private _outgoingMsgId: number;
  private _outgoingAckId: number;

  send(buffer: VSBuffer): void {
    const myId = ++this._outgoingMsgId;
    const msg = new ProtocolMessage(
      ProtocolMessageType.Regular,
      myId,
      this._incomingAckId,
      buffer
    );
    this._outgoingUnackMsg.push(msg);  // Keep until ACKed
    this._socketWriter.write(msg);
  }

  // When ACK received
  _receiveMessage(msg: ProtocolMessage): void {
    if (msg.ack > this._outgoingAckId) {
      this._outgoingAckId = msg.ack;
      // Remove acknowledged messages
      while (this._outgoingUnackMsg.peek()?.id <= msg.ack) {
        this._outgoingUnackMsg.pop();
      }
    }
  }
}
```

## Timeout Detection

### Conditions for Timeout

```typescript
if (
  timeSinceOldestUnacknowledgedMsg >= 20000 &&  // 20s
  timeSinceLastReceivedSomeData >= 20000 &&     // 20s
  timeSinceLastTimeout >= 20000                  // 20s
) {
  // Check CPU load (avoid false positives)
  if (!loadEstimator.hasHighLoad()) {
    this._onSocketTimeout.fire({
      unacknowledgedMsgCount,
      timeSinceOldestUnacknowledgedMsg,
      timeSinceLastReceivedSomeData
    });
  }
}
```

### Keep-Alive

```typescript
// Send every 5 seconds
setInterval(() => {
  const msg = new ProtocolMessage(
    ProtocolMessageType.KeepAlive,
    0,
    this._incomingAckId,
    emptyBuffer
  );
  this._socketWriter.write(msg);
}, 5000);
```

## Reconnection Flow

### 1. Connection Lost

```
Client detects socket close/timeout
    ↓
Client keeps PersistentProtocol state
    ↓
Client attempts reconnection
```

### 2. Reconnection Request

```typescript
// Client connects with same token
GET /?reconnectionToken=abc123&reconnection=true
```

### 3. Server Validates

```typescript
if (isReconnection) {
  if (!this._managementConnections[reconnectionToken]) {
    if (!this._allReconnectionTokens.has(reconnectionToken)) {
      // Never seen this token
      return reject("Unknown reconnection token (never seen)");
    } else {
      // Token expired
      return reject("Unknown reconnection token (seen before)");
    }
  }
  
  // Accept reconnection
  protocol.sendControl(JSON.stringify({ type: 'ok' }));
  this._managementConnections[reconnectionToken]
    .acceptReconnection(remoteAddress, socket, dataChunk);
}
```

### 4. State Restoration

```typescript
// Server side
acceptReconnection(socket, dataChunk) {
  this._protocol.beginAcceptReconnection(socket, dataChunk);
  this._protocol.endAcceptReconnection();
}

// PersistentProtocol
endAcceptReconnection() {
  // Re-send ACK
  const ackMsg = new ProtocolMessage(
    ProtocolMessageType.Ack,
    0,
    this._incomingAckId,
    emptyBuffer
  );
  this._socketWriter.write(ackMsg);

  // Replay unacknowledged messages
  for (const msg of this._outgoingUnackMsg.toArray()) {
    this._socketWriter.write(msg);
  }
}
```

## Grace Periods

### Normal Grace Period: 3 Hours

```typescript
ReconnectionGraceTime = 3 * 60 * 60 * 1000  // 3 hours
```

Server keeps connection state for 3 hours after disconnect.

### Short Grace Period: 5 Minutes

```typescript
ReconnectionShortGraceTime = 5 * 60 * 1000  // 5 minutes
```

Triggered when new connection arrives:

```typescript
// New connection shortens grace for disconnected ones
for (const conn of this._managementConnections) {
  conn.shortenReconnectionGraceTimeIfNecessary();
}
```

## Message Types

```typescript
enum ProtocolMessageType {
  None = 0,
  Regular = 1,        // Normal data message
  Control = 2,        // Control message (handshake)
  Ack = 3,            // Pure acknowledgment
  Disconnect = 5,     // Graceful disconnect
  ReplayRequest = 6,  // Request message replay
  Pause = 7,          // Pause writing
  Resume = 8,         // Resume writing
  KeepAlive = 9       // Keep connection alive
}
```

## Flow Control

### Pause/Resume

```typescript
// Server overwhelmed
protocol.sendPause();
// Client stops sending

// Server ready
protocol.sendResume();
// Client resumes
```

### Replay Request

```typescript
// Client missed messages
if (msg.id !== this._incomingMsgId + 1) {
  // Request replay
  this._socketWriter.write(new ProtocolMessage(
    ProtocolMessageType.ReplayRequest,
    0, 0, emptyBuffer
  ));
}

// Server replays
case ProtocolMessageType.ReplayRequest:
  for (const msg of this._outgoingUnackMsg.toArray()) {
    this._socketWriter.write(msg);
  }
```

## Error Handling

### Socket Errors

```typescript
socket.onClose((event) => {
  if (event.type === SocketCloseEventType.NodeSocketCloseEvent) {
    if (event.hadError) {
      // Transmission error
      console.error(event.error);
    }
  } else if (event.type === SocketCloseEventType.WebSocketCloseEvent) {
    // WebSocket close
    console.log(`Code: ${event.code}, Reason: ${event.reason}`);
  }
});
```

### Protocol Errors

```typescript
// Malformed message
try {
  msg = JSON.parse(raw.toString());
} catch (err) {
  return rejectConnection("Malformed message");
}

// Invalid message type
if (msg.type !== 'auth') {
  return rejectConnection("Invalid first message");
}
```

### IPC Errors

```typescript
// Request timeout
const timeout = setTimeout(() => {
  reject(new Error('Request timeout'));
}, 60000);

// Request error
try {
  const result = await channel.call(method, args);
} catch (error) {
  if (error.code === 'ENOENT') {
    // Resource not found
  } else if (error.code === 'EACCES') {
    // Permission denied
  }
}
```

## Graceful Disconnect

```typescript
// Client initiates disconnect
protocol.sendDisconnect();
protocol.dispose();
socket.end();

// Server receives
case ProtocolMessageType.Disconnect:
  this._onDidDispose.fire();
  // Clean up connection state
```

## Server Auto-Shutdown

```typescript
// After last extension host closes
if (!hasActiveExtHosts) {
  // Wait 5 minutes
  setTimeout(() => {
    if (!hasActiveExtHosts) {
      process.exit(0);
    }
  }, SHUTDOWN_TIMEOUT);  // 5 minutes
}
```

## Code References

- `src/vs/base/parts/ipc/common/ipc.net.ts` - PersistentProtocol
- `src/vs/server/node/remoteExtensionHostAgentServer.ts` - Reconnection handling
- `src/vs/server/node/remoteExtensionManagement.ts` - ManagementConnection
- `src/vs/server/node/extensionHostConnection.ts` - ExtensionHostConnection
