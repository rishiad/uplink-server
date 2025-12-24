# Terminal Protocol

VSCode Remote terminals use **PTY (Pseudo-Terminal)** processes running on the server, with I/O streamed to the client.

## Architecture

```
┌─────────────────────────────────────────────┐
│ VSCode Client                               │
│  ├─ Terminal UI (xterm.js)                  │
│  └─ Sends input, receives output            │
└─────────────────────────────────────────────┘
                    │
                    │ IPC over WebSocket
                    ▼
┌─────────────────────────────────────────────┐
│ Remote Server                               │
│  ├─ RemoteTerminalChannel                   │
│  └─ PtyHostService (proxy)                  │
└─────────────────────────────────────────────┘
                    │
                    │ IPC (separate process)
                    ▼
┌─────────────────────────────────────────────┐
│ PTY Host Process                            │
│  ├─ Manages PTY processes                   │
│  ├─ Spawns shells (bash, zsh, etc.)         │
│  └─ Handles I/O buffering                   │
└─────────────────────────────────────────────┘
                    │
                    │ PTY
                    ▼
┌─────────────────────────────────────────────┐
│ Shell Process (bash/zsh/fish)               │
│  └─ Runs on remote server                   │
└─────────────────────────────────────────────┘
```

## IPC Channel: `remoteterminal`

**Server**: `RemoteTerminalChannel` (remoteTerminalChannel.ts)

### Operations

#### Terminal Lifecycle
- `CreateProcess` - Create new terminal
- `AttachToProcess` - Attach to existing terminal
- `DetachFromProcess` - Detach from terminal
- `Start` - Start terminal process
- `Shutdown` - Stop terminal

#### I/O Operations
- `Input` - Send user input to terminal
- `ProcessBinary` - Send binary data
- `AcknowledgeDataEvent` - Flow control

#### Terminal Control
- `Resize` - Change terminal dimensions
- `SendSignal` - Send signal (SIGINT, SIGTERM, etc.)
- `ClearBuffer` - Clear scrollback

#### State Management
- `ListProcesses` - List active terminals
- `GetCwd` - Get current working directory
- `GetInitialCwd` - Get starting directory
- `SerializeTerminalState` - Save terminal state
- `ReviveTerminalProcesses` - Restore terminals

### Events

- `OnProcessDataEvent` - Terminal output
- `OnProcessReadyEvent` - Terminal ready
- `OnProcessExitEvent` - Terminal exited
- `OnProcessReplayEvent` - Replay buffered output
- `OnDidChangeProperty` - Property changed (title, cwd, etc.)

## Terminal Creation Flow

### 1. Client Requests Terminal

```typescript
// Client calls CreateProcess
{
  type: RequestType.Promise,
  channelName: "remoteterminal",
  methodName: "CreateProcess",
  args: [{
    shellLaunchConfig: {
      executable: "/bin/bash",
      args: ["-l"],
      cwd: "/home/user/project",
      env: { ... }
    },
    cols: 80,
    rows: 24,
    workspaceId: "workspace-123",
    workspaceName: "my-project"
  }]
}
```

### 2. Server Processes Request

**remoteTerminalChannel.ts**:
```typescript
async _createProcess(args) {
  // Build environment
  const baseEnv = await buildUserEnvironment(...);
  
  // Resolve variables in shell config
  const initialCwd = await getCwd(shellLaunchConfig, ...);
  
  // Apply extension environment variables
  mergedCollection.applyToProcessEnvironment(env, ...);
  
  // Create PTY via PtyHostService
  const persistentProcessId = await this._ptyHostService.createProcess(
    shellLaunchConfig, initialCwd, cols, rows, env, ...
  );
  
  return { persistentTerminalId: persistentProcessId };
}
```

### 3. PTY Host Spawns Shell

**PtyHostService** forwards to separate **PTY Host Process**:
- Spawns shell with `node-pty`
- Sets up PTY with specified dimensions
- Configures environment variables
- Returns process ID

### 4. Client Receives Terminal ID

```typescript
{
  type: ResponseType.PromiseSuccess,
  id: 42,
  result: {
    persistentTerminalId: 5,
    resolvedShellLaunchConfig: { ... }
  }
}
```

## Terminal I/O Flow

### User Types in Terminal

```
User types "ls -la"
    ↓
xterm.js captures input
    ↓
Client sends Input request
    ↓
IPC: [100, id, "remoteterminal", "Input", [5, "ls -la\r"]]
    ↓
RemoteTerminalChannel.call("Input", [5, "ls -la\r"])
    ↓
PtyHostService.input(5, "ls -la\r")
    ↓
PTY Host writes to PTY
    ↓
Shell receives input
```

### Shell Produces Output

```
Shell writes output
    ↓
PTY Host reads from PTY
    ↓
PtyHostService.onProcessData event
    ↓
RemoteTerminalChannel forwards event
    ↓
IPC: [204, "remoteterminal", "OnProcessDataEvent", { id: 5, data: "..." }]
    ↓
Client receives output
    ↓
xterm.js renders output
    ↓
User sees result
```

## PTY Host Process

Separate Node.js process for isolation and stability.

### Why Separate Process?

1. **Isolation**: PTY crashes don't affect main server
2. **Resource management**: Easier to restart/recover
3. **Security**: Additional process boundary
4. **Performance**: Dedicated event loop for I/O

### Communication

**PtyHostService** (in main server) ↔ **PTY Host Process**:
- IPC via Node.js `child_process` messaging
- ProxyChannel pattern for transparent RPC
- Heartbeat monitoring (every 5s)
- Auto-restart on crash (max 5 times)

### Heartbeat Monitoring

```typescript
// Server monitors PTY host health
heartbeatService.onBeat(() => this._handleHeartbeat());

// Timeouts:
// - First warning: 60s
// - Unresponsive: 120s
// - Create process timeout: 10s
```

## Terminal Persistence

Terminals survive client disconnections.

### Serialization

```typescript
// Save terminal state
await serializeTerminalState([terminalId1, terminalId2]);

// Returns serialized state with:
// - Process ID
// - Shell launch config
// - Current working directory
// - Environment variables
// - Scrollback buffer
```

### Revival

```typescript
// Restore terminals on reconnect
await reviveTerminalProcesses(workspaceId, serializedState, locale);

// PTY Host:
// - Checks if processes still exist
// - Reattaches to running processes
// - Replays buffered output
```

## Flow Control

Prevents overwhelming client with output.

### Acknowledgment System

```typescript
// Client acknowledges received data
await acknowledgeDataEvent(terminalId, charCount);

// PTY Host:
// - Tracks unacknowledged bytes
// - Pauses reading from PTY if buffer full
// - Resumes when client acknowledges
```

### Buffering

- PTY Host buffers output during disconnection
- Replays buffer on reconnect (via `OnProcessReplayEvent`)
- Configurable scrollback size (default: 100 lines)

## Environment Variables

### Base Environment

```typescript
// Built from:
// 1. Server process.env
// 2. User shell environment (if useShellEnvironment: true)
// 3. Resolved variables (${workspaceFolder}, etc.)
// 4. Extension contributions
```

### Extension Contributions

Extensions can modify terminal environment:

```typescript
// Extension API
const collection = vscode.window.createEnvironmentVariableCollection();
collection.replace('PATH', '/custom/bin:${env:PATH}');
collection.append('MY_VAR', ':extra');
```

Applied during terminal creation via `MergedEnvironmentVariableCollection`.

## Terminal Profiles

Predefined shell configurations.

### Detection

```typescript
// Server detects available shells
await getProfiles(workspaceId, profiles, defaultProfile, true);

// Returns:
[
  { profileName: "bash", path: "/bin/bash", isDefault: true },
  { profileName: "zsh", path: "/usr/bin/zsh" },
  { profileName: "fish", path: "/usr/bin/fish" }
]
```

### Platform-Specific

- **Linux/macOS**: Scans common shell paths
- **Windows**: Detects PowerShell, CMD, Git Bash, WSL

## Special Features

### Shell Integration

```typescript
shellLaunchConfig.shellIntegrationEnvironmentReporting = true;

// Injects shell integration scripts
// Enables features:
// - Command detection
// - Exit code tracking
// - CWD tracking
// - Command history
```

### CLI Server

Each terminal gets IPC handle for CLI commands:

```typescript
env.VSCODE_IPC_HOOK_CLI = ipcHandlePath;

// Enables `code` command in terminal:
// $ code file.txt  # Opens in VSCode
// $ code --goto file.txt:10  # Opens at line 10
```

### WSL Support

```typescript
// Convert paths between Windows and WSL
await getWslPath('/mnt/c/Users/name', 'unix-to-win');
// Returns: C:\Users\name
```

## Performance Considerations

### Latency

- **Input latency**: ~10-20ms (network + processing)
- **Output latency**: ~10-30ms (depends on output volume)
- **Resize latency**: ~5-10ms

### Throughput

- **Typical**: ~1-5 MB/s (text output)
- **Burst**: ~10-20 MB/s (large command output)
- **Limited by**: Network bandwidth, WebSocket framing

### Optimization

- Binary data transfer (not base64)
- Acknowledgment-based flow control
- Output buffering during disconnection
- Efficient event batching

## Code References

### Server
- `src/vs/server/node/remoteTerminalChannel.ts` - IPC channel
- `src/vs/platform/terminal/node/ptyHostService.ts` - PTY host proxy
- `src/vs/platform/terminal/node/ptyHost.ts` - PTY host process

### Common
- `src/vs/platform/terminal/common/terminal.ts` - Interfaces
- `src/vs/workbench/contrib/terminal/common/remote/terminal.ts` - Remote types
- `src/vs/workbench/contrib/terminal/common/terminalEnvironment.ts` - Environment handling
