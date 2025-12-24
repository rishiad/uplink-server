# Extension Host Debug Protocol

Debugging extensions running in the remote extension host.

## IPC Channel: `ExtensionHostDebugBroadcast`

**Server**: `ExtensionHostDebugBroadcastChannel`  
**Location**: `vscode-server/src/vs/platform/debug/common/extensionHostDebugIpc.ts`

## Architecture

```
┌─────────────────────────────────────────────┐
│ VSCode Client                               │
│  ├─ Debug UI                                │
│  └─ Debug Adapter Protocol (DAP)            │
└─────────────────────────────────────────────┘
                    │
                    │ IPC
                    ▼
┌─────────────────────────────────────────────┐
│ Remote Server                               │
│  └─ ExtensionHostDebugBroadcastChannel      │
└─────────────────────────────────────────────┘
                    │
                    │ Node.js Inspector
                    ▼
┌─────────────────────────────────────────────┐
│ Extension Host Process                      │
│  └─ --inspect=<port>                        │
└─────────────────────────────────────────────┘
```

## Debug Port Assignment

When extension host starts with debugging:

```typescript
// Client requests debug port
const startParams: IRemoteExtensionHostStartParams = {
  port: 9229,      // Requested debug port
  break: true,     // Break on start
  debugId: "uuid"  // Debug session ID
};

// Server finds free port
const freePort = await findFreePort(startParams.port, 10, 5000);

// Extension host spawned with inspector
const execArgv = [
  `--inspect${startParams.break ? '-brk' : ''}=${freePort}`,
  '--experimental-network-inspection'
];
```

## Operations

### `reload(sessionId)`

Reload extension host for debug session.

```typescript
// Client requests reload
await channel.call('reload', [sessionId]);

// Server broadcasts to all clients
this._onReload.fire({ sessionId });
```

### `close(sessionId)`

Close debug session.

```typescript
await channel.call('close', [sessionId]);
```

### `attachSession(sessionId, port)`

Attach debugger to extension host.

```typescript
await channel.call('attachSession', [sessionId, 9229]);
```

## Events

### `onAttachSession`

Debug session attached.

```typescript
channel.listen('onAttachSession').subscribe(({ sessionId, port }) => {
  // Connect debugger to port
});
```

### `onTerminateSession`

Debug session terminated.

```typescript
channel.listen('onTerminateSession').subscribe(({ sessionId }) => {
  // Clean up debug session
});
```

### `onLogToSession`

Log message to debug console.

```typescript
channel.listen('onLogToSession').subscribe(({ sessionId, message }) => {
  // Display in debug console
});
```

## Debug Flow

### 1. Start Debug Session

```
Client: "Debug Extension Host"
    ↓
Client sends ConnectionType.ExtensionHost with debug params
    ↓
Server finds free debug port
    ↓
Server spawns extension host with --inspect-brk=<port>
    ↓
Server returns { debugPort: <port> }
    ↓
Client attaches debugger to port via tunnel
```

### 2. Attach Debugger

```typescript
// Server response includes debug port
{ type: "ok", debugPort: 9229 }

// Client creates tunnel to debug port
const tunnel = await createTunnel(remoteHost, 9229);

// Client connects DAP to tunnel
debugAdapter.connect(tunnel.localPort);
```

### 3. Debug Session

```
Debugger ←→ Extension Host (via Node.js Inspector Protocol)
    │
    │ Breakpoints, stepping, variables
    │
    ▼
Extension code execution paused/resumed
```

## Broadcast Pattern

Channel uses broadcast pattern for multi-client scenarios:

```typescript
class ExtensionHostDebugBroadcastChannel implements IServerChannel {
  private readonly _onReload = new Emitter<{ sessionId: string }>();
  private readonly _onClose = new Emitter<{ sessionId: string }>();
  private readonly _onAttachSession = new Emitter<...>();
  private readonly _onTerminateSession = new Emitter<...>();
  private readonly _onLogToSession = new Emitter<...>();

  call(ctx, command, args) {
    switch (command) {
      case 'reload':
        this._onReload.fire({ sessionId: args[0] });
        break;
      // ...
    }
  }

  listen(ctx, event) {
    switch (event) {
      case 'reload': return this._onReload.event;
      // ...
    }
  }
}
```

## Tunnel Connection

Debug uses `ConnectionType.Tunnel` for port forwarding:

```typescript
// Client requests tunnel
{
  type: "connectionType",
  desiredConnectionType: 3,  // Tunnel
  args: {
    host: "localhost",
    port: 9229
  }
}

// Server creates TCP tunnel
const localSocket = await net.createConnection({ host, port });
remoteSocket.pipe(localSocket);
localSocket.pipe(remoteSocket);
```

## Code References

- `src/vs/platform/debug/common/extensionHostDebugIpc.ts` - Channel implementation
- `src/vs/platform/debug/common/extensionHostDebug.ts` - Interfaces
- `src/vs/server/node/extensionHostConnection.ts` - Debug port handling
- `src/vs/server/node/remoteExtensionHostAgentServer.ts` - Tunnel creation
