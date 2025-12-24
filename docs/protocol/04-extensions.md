# Extension Host Architecture

VSCode extensions run in separate **Extension Host** processes, isolated from the main UI for stability and security.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│ VSCode Client (Browser/Electron)                            │
│  ├─ UI Process                                              │
│  └─ Connects to Remote Server via WebSocket                 │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ WebSocket over SSH tunnel
                          ▼
┌─────────────────────────────────────────────────────────────┐
│ Remote Server (uplink-server)                               │
│  ├─ Management Connection (connection type 1)               │
│  ├─ Extension Host Connection (connection type 2)           │
│  └─ Spawns Extension Host Process                           │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ IPC / Socket
                          ▼
┌─────────────────────────────────────────────────────────────┐
│ Extension Host Process (Node.js)                            │
│  ├─ Loads and activates extensions                          │
│  ├─ Runs extension code in isolated context                 │
│  └─ Communicates via RPC protocol                           │
└─────────────────────────────────────────────────────────────┘
```

## Connection Types

When client connects to server, it specifies connection type in handshake:

1. **Management Connection** (`desiredConnectionType: 1`)
   - Main control channel
   - Handles filesystem, terminal, configuration
   - One per client session

2. **Extension Host Connection** (`desiredConnectionType: 2`)
   - Dedicated channel for extension host process
   - Bidirectional RPC communication
   - Multiple possible (one per extension host)

3. **Tunnel Connection** (`desiredConnectionType: 3`)
   - Port forwarding for debugging/services

## Extension Host Lifecycle

### 1. Spawning Extension Host

**Server**: `extensionHostConnection.ts`
```typescript
// Fork extension host process
const args = ['--type=extensionHost', '--transformURIs'];
this._extensionHostProcess = cp.fork(
  'bootstrap-fork',
  args,
  { env, execArgv, silent: true }
);
```

**Entry Point**: `vs/workbench/api/node/extensionHostProcess.ts`

### 2. Socket Handoff

Two modes depending on platform:

**Direct Socket Transfer** (Linux/macOS):
```typescript
// Server sends socket file descriptor to extension host
const msg: IExtHostSocketMessage = {
  type: 'VSCODE_EXTHOST_IPC_SOCKET',
  initialDataChunk: base64Data,
  skipWebSocketFrames: true,
  permessageDeflate: false
};
extensionHostProcess.send(msg, socket);
```

**Named Pipe** (Windows with `--socket-path`):
```typescript
// Server creates named pipe, extension host connects
const pipeName = createRandomIPCHandle();
const namedPipeServer = net.createServer();
namedPipeServer.listen(pipeName);
```

### 3. Extension Loading

**Server**: `remoteExtensionsScanner.ts`
- Scans extension directories
- Validates extension manifests
- Returns extension metadata to client

**Extension Host**: `extHostExtensionService.ts`
- Receives extension list from client
- Activates extensions based on activation events
- Provides extension API (`vscode.*`)

## RPC Protocol

Extension host uses **RPC over IPC** for communication:

### Message Types

```typescript
// Request from client to extension host
{
  type: RequestType.Promise,        // 100
  id: number,
  channelName: string,
  methodName: string,
  args: any[]
}

// Response from extension host to client
{
  type: ResponseType.PromiseSuccess, // 201
  id: number,
  result: any
}
```

### Extension API Proxying

Extensions call `vscode.*` APIs, which are proxied to client:

```typescript
// Extension code
await vscode.window.showInformationMessage('Hello');

// Proxied as RPC call
{
  type: 100,
  channelName: 'window',
  methodName: 'showInformationMessage',
  args: ['Hello']
}
```

## IPC Channels for Extensions

Registered in `serverServices.ts`:

### `extensions` Channel
- **Purpose**: Extension management (install, uninstall, update)
- **Server**: `ExtensionManagementChannel`
- **Operations**: `install`, `uninstall`, `getInstalled`, `updateMetadata`

### `remoteextensionsscanner` Channel
- **Purpose**: Scan and list available extensions
- **Server**: `RemoteExtensionsScannerChannel`
- **Operations**: `scanExtensions`, `scanSingleExtension`

### `remoteextensionsenvironment` Channel
- **Purpose**: Extension host environment info
- **Server**: `RemoteAgentEnvironmentChannel`
- **Operations**: `getEnvironmentData`, `getDiagnosticInfo`

## Extension Activation

Extensions activate based on **activation events**:

```json
{
  "activationEvents": [
    "onLanguage:python",           // When Python file opened
    "onCommand:myext.doSomething", // When command invoked
    "onFileSystem:sftp",           // When SFTP filesystem used
    "*"                            // On startup (discouraged)
  ]
}
```

**Activation Flow**:
1. Client detects activation event (e.g., file opened)
2. Client sends activation request to extension host
3. Extension host loads extension module
4. Calls extension's `activate(context)` function
5. Extension registers commands, providers, etc.

## Extension API Surface

Extensions access remote resources through proxied APIs:

### Filesystem
```typescript
vscode.workspace.fs.readFile(uri)  // Proxied to server
vscode.workspace.fs.writeFile(uri, content)
```

### Terminal
```typescript
vscode.window.createTerminal()  // Creates terminal on server
terminal.sendText('ls -la')     // Executes on remote
```

### Language Features
```typescript
vscode.languages.registerCompletionItemProvider()
vscode.languages.registerHoverProvider()
// Extension runs on server, has direct filesystem access
```

## Performance Considerations

### What Runs Where

**Client Side**:
- UI rendering
- User input handling
- Syntax highlighting (TextMate grammars)

**Extension Host (Server)**:
- Extension code execution
- Language servers
- File system operations
- Git operations
- Debuggers

### Latency Impact

- **Low latency operations**: File reads, language features (server-local)
- **High latency operations**: UI updates, user prompts (round-trip to client)

## Code References

### Server
- `src/vs/server/node/extensionHostConnection.ts` - Extension host process management
- `src/vs/server/node/remoteExtensionsScanner.ts` - Extension scanning
- `src/vs/server/node/serverServices.ts` - IPC channel registration

### Extension Host
- `src/vs/workbench/api/node/extensionHostProcess.ts` - Entry point
- `src/vs/workbench/api/common/extHostExtensionService.ts` - Extension activation
- `src/vs/workbench/services/extensions/common/rpcProtocol.ts` - RPC implementation

### Protocol
- `src/vs/workbench/services/extensions/common/extensionHostProtocol.ts` - Message types
- `src/vs/base/parts/ipc/common/ipc.ts` - IPC layer
