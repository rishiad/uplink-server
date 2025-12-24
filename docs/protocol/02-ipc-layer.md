# IPC Layer

Channel-based RPC system built on top of PersistentProtocol.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Application Layer                                      │
│  - FileSystemProvider                                   │
│  - TerminalService                                      │
│  - ExtensionManagementService                           │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│  IPC Channel Layer                                      │
│  Client: ChannelClient.getChannel(name)                │
│  Server: ChannelServer.registerChannel(name, channel)  │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│  Serialization Layer                                    │
│  - serialize(writer, data)                              │
│  - deserialize(reader)                                  │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│  PersistentProtocol                                     │
│  - send(buffer: VSBuffer)                               │
│  - onMessage: Event<VSBuffer>                           │
└─────────────────────────────────────────────────────────┘
```

**Location**: `uplink-server/vscode-server/src/vs/base/parts/ipc/common/ipc.ts`

## Request/Response Protocol

### Request Types

```typescript
enum RequestType {
  Promise = 100,        // Method call (returns promise)
  PromiseCancel = 101,  // Cancel method call
  EventListen = 102,    // Subscribe to event
  EventDispose = 103    // Unsubscribe from event
}
```

### Response Types

```typescript
enum ResponseType {
  Initialize = 200,       // Handshake
  PromiseSuccess = 201,   // Method result
  PromiseError = 202,     // Method error (Error object)
  PromiseErrorObj = 203,  // Method error (any object)
  EventFire = 204         // Event data
}
```

### Message Format

**Request**:
```typescript
// Promise request
[RequestType.Promise, id, channelName, methodName, args]

// Event listen
[RequestType.EventListen, id, channelName, eventName, args]

// Cancel
[RequestType.PromiseCancel, id]
```

**Response**:
```typescript
// Success
[ResponseType.PromiseSuccess, id, result]

// Error
[ResponseType.PromiseError, id, { message, name, stack }]

// Event
[ResponseType.EventFire, id, eventData]
```

## Serialization

**Location**: `ipc.ts:serialize()` and `deserialize()`

### Data Types

```typescript
enum DataType {
  Undefined = 0,
  String = 1,
  Buffer = 2,
  VSBuffer = 3,
  Array = 4,
  Object = 5,    // JSON-encoded
  Int = 6        // VQL-encoded
}
```

### Encoding Format

**String**: `[type:1][length:VQL][utf8-bytes]`
**Buffer**: `[type:1][length:VQL][raw-bytes]`
**Array**: `[type:1][length:VQL][item1][item2]...`
**Object**: `[type:1][length:VQL][json-utf8]`
**Int**: `[type:1][value:VQL]`

### Variable-Length Quantity (VQL)

Compact integer encoding:
- 1 byte: 0-127
- 2 bytes: 128-16,383
- 3 bytes: 16,384-2,097,151
- etc.

**Implementation**: `writeInt32VQL()` and `readIntVQL()`

## Channel System

### Client Side

```typescript
class ChannelClient {
  getChannel<T>(channelName: string): T {
    return {
      call(method: string, args?: any): Promise<any> {
        const id = this.lastRequestId++;
        this.sendRequest([RequestType.Promise, id, channelName, method], args);
        return this.waitForResponse(id);
      },
      
      listen(event: string, args?: any): Event<any> {
        const id = this.lastRequestId++;
        this.sendRequest([RequestType.EventListen, id, channelName, event], args);
        return this.createEventEmitter(id);
      }
    };
  }
}
```

**Location**: `ipc.ts:ChannelClient`

### Server Side

```typescript
class ChannelServer {
  registerChannel(channelName: string, channel: IServerChannel): void {
    this.channels.set(channelName, channel);
  }
  
  private onPromise(request: IRawPromiseRequest): void {
    const channel = this.channels.get(request.channelName);
    const promise = channel.call(ctx, request.name, request.arg, token);
    
    promise.then(
      data => this.sendResponse({ type: ResponseType.PromiseSuccess, id, data }),
      err => this.sendResponse({ type: ResponseType.PromiseError, id, data: err })
    );
  }
}
```

**Location**: `ipc.ts:ChannelServer`

## ProxyChannel Pattern

Automatic service wrapping without manual IPC code.

### Server: Wrap Service

```typescript
// Automatically creates channel from service
const channel = ProxyChannel.fromService(fileSystemProvider, disposables);
channelServer.registerChannel('fs', channel);
```

**Implementation**: `ipc.ts:ProxyChannel.fromService()`

**Rules**:
- Methods: Any function property
- Events: Properties starting with `on` + uppercase letter
- Dynamic events: `onDynamic*` methods returning events

### Client: Unwrap Service

```typescript
// Automatically creates proxy from channel
const fs = ProxyChannel.toService<IFileSystemProvider>(
  channelClient.getChannel('fs')
);

// Use like local service
await fs.readFile(uri);
fs.onDidChangeFile(e => console.log(e));
```

**Implementation**: `ipc.ts:ProxyChannel.toService()`

## Message Flow Example

### readFile Operation

```
Client                                    Server
──────────────────────────────────────────────────────────

1. Application calls
   fs.readFile(uri)
   ↓
2. ChannelClient serializes
   header = [100, 42, "fs", "readFile"]
   body = [{ scheme: "vscode-remote", path: "/file" }]
   ↓
3. BufferWriter encodes
   writer.write(header) → [4, 100, 42, "fs", "readFile"]
   writer.write(body) → [5, {...}]
   ↓
4. PersistentProtocol frames
   [Type=1, ID=1, ACK=0, Len=N][data]
   ↓
5. Send over WebSocket
                                          ↓
                                    6. PersistentProtocol receives
                                       ↓
                                    7. ChannelServer deserializes
                                       type=100, id=42, channel="fs"
                                       method="readFile", args=[uri]
                                       ↓
                                    8. Call service
                                       const data = await fs.readFile(uri)
                                       ↓
                                    9. Serialize response
                                       [201, 42, VSBuffer(data)]
                                       ↓
   ←─────────────────────────────  10. Send response
11. Deserialize
    ↓
12. Resolve promise
    ↓
13. Return to application
```

## IPCServer/IPCClient

High-level abstractions for multi-client scenarios.

### IPCServer

```typescript
class IPCServer<TContext> {
  constructor(onDidClientConnect: Event<ClientConnectionEvent>) {
    // Automatically creates ChannelServer per client
    // Routes calls to appropriate client
  }
  
  registerChannel(name: string, channel: IServerChannel<TContext>): void {
    // Registers channel for all clients
  }
  
  getChannel<T>(name: string, router: IClientRouter<TContext>): T {
    // Gets channel from specific client
  }
}
```

**Location**: `ipc.ts:IPCServer`

**Used by**: `serverServices.ts:SocketServer`

### IPCClient

```typescript
class IPCClient<TContext> {
  constructor(protocol: IMessagePassingProtocol, ctx: TContext) {
    // Creates both ChannelClient and ChannelServer
    // Bidirectional communication
  }
  
  getChannel<T>(name: string): T;
  registerChannel(name: string, channel: IServerChannel<TContext>): void;
}
```

**Location**: `ipc.ts:IPCClient`

## Error Handling

### Error Serialization

```typescript
// Server catches error
try {
  const result = await channel.call(method, args);
} catch (err) {
  if (err instanceof Error) {
    this.sendResponse({
      type: ResponseType.PromiseError,
      id,
      data: {
        message: err.message,
        name: err.name,
        stack: err.stack?.split('\n')
      }
    });
  }
}
```

### Error Deserialization

```typescript
// Client reconstructs error
const error = new Error(response.data.message);
error.stack = response.data.stack.join('\n');
error.name = response.data.name;
throw error;
```

## Cancellation

### Client Cancels

```typescript
const token = new CancellationTokenSource();
const promise = channel.call('method', args, token.token);

// Later...
token.cancel();  // Sends PromiseCancel message
```

### Server Handles Cancellation

```typescript
const cancellationTokenSource = new CancellationTokenSource();
const promise = channel.call(ctx, method, args, cancellationTokenSource.token);

// On cancel message
cancellationTokenSource.cancel();
```

## Performance Considerations

### Batching

Multiple IPC calls can be batched into single protocol message:
```typescript
// These may be batched
channel.call('stat', [uri1]);
channel.call('stat', [uri2]);
channel.call('stat', [uri3]);
// → Single TCP packet
```

### Pipelining

Multiple requests in flight simultaneously:
```typescript
const p1 = channel.call('readFile', [uri1]);  // id=1
const p2 = channel.call('readFile', [uri2]);  // id=2
const p3 = channel.call('readFile', [uri3]);  // id=3

// Responses can arrive out of order
await Promise.all([p1, p2, p3]);
```

### Binary Efficiency

- No JSON overhead
- VQL encoding for integers
- Direct buffer passing (zero-copy)
- ~50-70% smaller than JSON

## Code References

### Core IPC
- `vscode-server/src/vs/base/parts/ipc/common/ipc.ts` - Main IPC implementation
- `vscode-server/src/vs/base/parts/ipc/common/ipc.net.ts` - Protocol layer

### Server Setup
- `vscode-server/src/vs/server/node/serverServices.ts:setupServerServices()` - Channel registration
- `vscode-server/src/vs/server/node/serverServices.ts:SocketServer` - Multi-client server

### Usage Examples
- `vscode-server/src/vs/server/node/remoteFileSystemProviderServer.ts` - Filesystem channel
- `vscode-server/src/vs/server/node/remoteTerminalChannel.ts` - Terminal channel
- `vscode-server/src/vs/server/node/remoteAgentEnvironmentImpl.ts` - Environment channel
