# All IPC Channels

Complete registry of all IPC channels in the VSCode Remote protocol.

**Registration Location**: `uplink-server/vscode-server/src/vs/server/node/serverServices.ts:setupServerServices()`

## Channel Registry

### 1. `logger`

**Purpose**: Remote logging service  
**Server**: `LoggerChannel`  
**Location**: `vscode-server/src/vs/platform/log/common/logIpc.ts`

**Operations**:
- `createLogger(resource, options)` - Create logger instance
- `setLevel(resource, level)` - Set log level
- `getLevel(resource)` - Get log level

**Events**:
- `onDidChangeLogLevel` - Log level changed
- `onDidChangeVisibility` - Logger visibility changed

---

### 2. `telemetry`

**Purpose**: Telemetry data collection  
**Server**: `ServerTelemetryChannel`  
**Location**: `vscode-server/src/vs/platform/telemetry/common/remoteTelemetryChannel.ts`

**Operations**:
- `publicLog(eventName, data)` - Send telemetry event
- `publicLog2(eventName, data)` - Send classified event
- `getTelemetryLevel()` - Get telemetry level
- `setTelemetryLevel(level)` - Set telemetry level

**Events**:
- `onDidChangeTelemetryLevel` - Telemetry level changed

---

### 3. `request`

**Purpose**: HTTP request service  
**Server**: `RequestChannel`  
**Location**: `vscode-server/src/vs/platform/request/common/requestIpc.ts`

**Operations**:
- `request(options, token)` - Make HTTP request
- `resolveProxy(url)` - Resolve proxy for URL

---

### 4. `remoteextensionsenvironment`

**Purpose**: Remote environment information  
**Server**: `RemoteAgentEnvironmentChannel`  
**Location**: `vscode-server/src/vs/server/node/remoteAgentEnvironmentImpl.ts`

**Operations**:
- `getEnvironmentData()` - Get environment info (paths, OS, arch)
- `getExtensionHostExitInfo(token)` - Get extension host exit info
- `getDiagnosticInfo()` - Get diagnostic information
- `disableTelemetry()` - Disable telemetry

**Events**:
- `onDidChangeExtensionHostStatus` - Extension host status changed

**Returns**:
```typescript
{
  pid: number;
  connectionToken: string;
  appRoot: URI;
  settingsPath: URI;
  logsPath: URI;
  extensionsPath: URI;
  extensionHostLogsPath: URI;
  globalStorageHome: URI;
  workspaceStorageHome: URI;
  userHome: URI;
  os: OperatingSystem;
  arch: string;
  marks: PerformanceMark[];
}
```

---

### 5. `vscode.remoteFileSystemProvider`

**Purpose**: Filesystem operations  
**Server**: `RemoteAgentFileSystemProviderChannel`  
**Location**: `vscode-server/src/vs/server/node/remoteFileSystemProviderServer.ts`

**See**: [Filesystem Protocol](03-filesystem.md)

**Operations**:
- `stat(uri)` - Get file metadata
- `readFile(uri)` - Read file contents
- `writeFile(uri, content, options)` - Write file
- `delete(uri, options)` - Delete file/directory
- `rename(oldUri, newUri, options)` - Rename/move
- `copy(source, dest, options)` - Copy file/directory
- `readDirectory(uri)` - List directory
- `createDirectory(uri)` - Create directory
- `watch(uri, options)` - Watch for changes

**Events**:
- `onDidChangeFile` - File/directory changed
- `onDidWatchError` - Watch error occurred

---

### 6. `remoteterminal`

**Purpose**: Terminal/PTY management  
**Server**: `RemoteTerminalChannel`  
**Location**: `vscode-server/src/vs/server/node/remoteTerminalChannel.ts`

**Operations**:
- `createTerminal(options)` - Create terminal instance
- `sendText(terminalId, text)` - Send text to terminal
- `resize(terminalId, cols, rows)` - Resize terminal
- `dispose(terminalId)` - Dispose terminal

**Events**:
- `onProcessData` - Terminal output data
- `onProcessExit` - Terminal process exited
- `onProcessReady` - Terminal process ready
- `onProcessTitleChanged` - Terminal title changed

---

### 7. `extensions`

**Purpose**: Extension management  
**Server**: `ExtensionManagementChannel`  
**Location**: `vscode-server/src/vs/platform/extensionManagement/common/extensionManagementIpc.ts`

**Operations**:
- `install(vsix)` - Install extension from VSIX
- `installFromGallery(extension)` - Install from marketplace
- `uninstall(extension)` - Uninstall extension
- `getInstalled(type)` - Get installed extensions
- `updateMetadata(local, metadata)` - Update extension metadata
- `getExtensionsControlManifest()` - Get control manifest

**Events**:
- `onInstallExtension` - Extension installation started
- `onDidInstallExtensions` - Extensions installed
- `onUninstallExtension` - Extension uninstall started
- `onDidUninstallExtension` - Extension uninstalled

---

### 8. `remoteextensionsscanner`

**Purpose**: Extension scanning  
**Server**: `RemoteExtensionsScannerChannel`  
**Location**: `vscode-server/src/vs/server/node/remoteExtensionsScanner.ts`

**Operations**:
- `scanExtensions(type, profileLocation)` - Scan extensions
- `scanSystemExtensions()` - Scan system extensions
- `scanUserExtensions()` - Scan user extensions
- `scanMetadata(extensionLocation)` - Scan extension metadata
- `scanExtensionsUnderDevelopment()` - Scan dev extensions

**Events**:
- `onDidChangeCache` - Extension cache changed

---

### 9. `userDataProfiles`

**Purpose**: User data profile management  
**Server**: `RemoteUserDataProfilesServiceChannel`  
**Location**: `vscode-server/src/vs/platform/userDataProfile/common/userDataProfileIpc.ts`

**Operations**:
- `createProfile(name, options)` - Create profile
- `removeProfile(profile)` - Remove profile
- `updateProfile(profile, options)` - Update profile
- `getProfiles()` - Get all profiles
- `getDefaultProfile()` - Get default profile

**Events**:
- `onDidChangeProfiles` - Profiles changed

---

### 10. `download`

**Purpose**: File download service  
**Server**: `DownloadChannel` (from client)  
**Location**: `vscode-server/src/vs/platform/download/common/downloadIpc.ts`

**Operations**:
- `download(uri, target, options)` - Download file

**Events**:
- `onDidDownload` - Download completed

---

### 11. `ExtensionHostDebugBroadcast`

**Purpose**: Extension host debugging  
**Server**: `ExtensionHostDebugBroadcastChannel`  
**Location**: `vscode-server/src/vs/platform/debug/common/extensionHostDebugIpc.ts`

**Operations**:
- `reload(sessionId)` - Reload debug session
- `close(sessionId)` - Close debug session
- `attachSession(sessionId, port)` - Attach debugger

**Events**:
- `onAttachSession` - Debug session attached
- `onTerminateSession` - Debug session terminated
- `onLogToSession` - Log message to session

---

### 12. `mcpManagement`

**Purpose**: MCP (Model Context Protocol) server management  
**Server**: `McpManagementChannel`  
**Location**: `vscode-server/src/vs/platform/mcp/common/mcpManagementIpc.ts`

**Operations**:
- `install(vsix)` - Install MCP server
- `uninstall(server)` - Uninstall MCP server
- `getInstalled()` - Get installed MCP servers

**Events**:
- `onInstallMcpServer` - MCP server installation started
- `onDidInstallMcpServer` - MCP server installed

---

### 13. `NativeMcpDiscoveryHelper`

**Purpose**: MCP server discovery  
**Server**: `NativeMcpDiscoveryHelperChannel`  
**Location**: `vscode-server/src/vs/platform/mcp/node/nativeMcpDiscoveryHelperChannel.ts`

**Operations**:
- Discovery and enumeration of MCP servers

---

## Channel Registration Code

```typescript
// serverServices.ts:setupServerServices()

// Core services
socketServer.registerChannel('logger', new LoggerChannel(...));
socketServer.registerChannel('telemetry', new ServerTelemetryChannel(...));
socketServer.registerChannel('request', new RequestChannel(...));

// Remote environment
socketServer.registerChannel('remoteextensionsenvironment', 
  new RemoteAgentEnvironmentChannel(...));

// Filesystem
socketServer.registerChannel(REMOTE_FILE_SYSTEM_CHANNEL_NAME, 
  new RemoteAgentFileSystemProviderChannel(...));
// REMOTE_FILE_SYSTEM_CHANNEL_NAME = 'vscode.remoteFileSystemProvider'

// Terminal
socketServer.registerChannel(REMOTE_TERMINAL_CHANNEL_NAME, 
  new RemoteTerminalChannel(...));
// REMOTE_TERMINAL_CHANNEL_NAME = 'remoteterminal'

// Extensions
socketServer.registerChannel('extensions', 
  new ExtensionManagementChannel(...));
socketServer.registerChannel(RemoteExtensionsScannerChannelName, 
  new RemoteExtensionsScannerChannel(...));
// RemoteExtensionsScannerChannelName = 'remoteextensionsscanner'

// User data
socketServer.registerChannel('userDataProfiles', 
  new RemoteUserDataProfilesServiceChannel(...));

// Downloads (from client)
const downloadChannel = socketServer.getChannel('download', router);

// Debugging
socketServer.registerChannel(ExtensionHostDebugBroadcastChannel.ChannelName, 
  new ExtensionHostDebugBroadcastChannel());

// MCP
socketServer.registerChannel('mcpManagement', 
  new McpManagementChannel(...));
socketServer.registerChannel(NativeMcpDiscoveryHelperChannelName, 
  new NativeMcpDiscoveryHelperChannel(...));
```

## Channel Usage Pattern

### Client Side

```typescript
// Get channel
const channel = channelClient.getChannel('vscode.remoteFileSystemProvider');

// Call method
const stat = await channel.call('stat', [uri]);

// Listen to events
const disposable = channel.listen('onDidChangeFile').subscribe(changes => {
  console.log(changes);
});
```

### Server Side

```typescript
// Implement channel
class MyChannel implements IServerChannel {
  call(ctx, command, arg, token) {
    switch (command) {
      case 'myMethod':
        return this.myMethod(arg);
    }
  }
  
  listen(ctx, event, arg) {
    switch (event) {
      case 'onMyEvent':
        return this.onMyEvent;
    }
  }
}

// Register channel
socketServer.registerChannel('myChannel', new MyChannel());
```

## Adding Custom Channels

To add a custom channel:

1. **Define interface** in common code
2. **Implement server channel** in server code
3. **Register channel** in `serverServices.ts`
4. **Get channel** on client side
5. **Use ProxyChannel** for automatic wrapping (optional)

Example:
```typescript
// 1. Interface
interface IMyService {
  doSomething(arg: string): Promise<string>;
  onSomethingHappened: Event<string>;
}

// 2. Server implementation
class MyServiceChannel implements IServerChannel {
  constructor(private service: IMyService) {}
  
  call(ctx, command, arg) {
    return this.service[command](arg);
  }
  
  listen(ctx, event) {
    return this.service[event];
  }
}

// 3. Register
socketServer.registerChannel('myService', new MyServiceChannel(myService));

// 4. Client usage
const channel = client.getChannel('myService');
const result = await channel.call('doSomething', ['arg']);

// 5. Or use ProxyChannel
const service = ProxyChannel.toService<IMyService>(channel);
const result = await service.doSomething('arg');
```

## Performance Considerations

### Channel Overhead

Each channel call involves:
1. Serialization (~1ms)
2. IPC framing (~1ms)
3. Protocol framing (~1ms)
4. Network transmission (variable)
5. Deserialization (~1ms)

**Total**: ~5-10ms + network latency

### Optimization Strategies

1. **Batch operations**: Combine multiple calls
2. **Cache results**: Avoid redundant calls
3. **Use events**: Push updates instead of polling
4. **Binary data**: Use VSBuffer for large data
5. **Compression**: Enable WebSocket compression

## Code References

- `vscode-server/src/vs/server/node/serverServices.ts` - Channel registration
- `vscode-server/src/vs/base/parts/ipc/common/ipc.ts` - IPC infrastructure
- Individual channel implementations in respective service directories
