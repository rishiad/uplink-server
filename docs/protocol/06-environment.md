# Remote Environment Protocol

Client queries server environment information during connection setup and for diagnostics.

## IPC Channel: `remoteextensionsenvironment`

**Server**: `RemoteAgentEnvironmentChannel` (remoteAgentEnvironmentImpl.ts)

## Operations

### `getEnvironmentData`

Returns comprehensive server environment information.

**Request**:
```typescript
{
  type: RequestType.Promise,
  channelName: "remoteextensionsenvironment",
  methodName: "getEnvironmentData",
  args: [{
    remoteAuthority: "ssh-remote+hostname",
    profile?: "profile-id"  // Optional user data profile
  }]
}
```

**Response**:
```typescript
{
  // Process info
  pid: 12345,
  connectionToken: "abc123...",
  
  // Paths (as URIs)
  appRoot: URI.file("/path/to/vscode-server"),
  settingsPath: URI.file("~/.vscode-server/data/Machine/settings.json"),
  mcpResource: URI.file("~/.vscode-server/data/mcp.json"),
  logsPath: URI.file("~/.vscode-server/data/logs/20240115T120000"),
  extensionHostLogsPath: URI.file("~/.vscode-server/data/logs/.../exthost1"),
  globalStorageHome: URI.file("~/.vscode-server/data/User/globalStorage"),
  workspaceStorageHome: URI.file("~/.vscode-server/data/User/workspaceStorage"),
  localHistoryHome: URI.file("~/.vscode-server/data/User/History"),
  userHome: URI.file("/home/user"),
  
  // System info
  os: 1,  // 1=Linux, 2=macOS, 3=Windows
  arch: "x64",  // or "arm64"
  
  // Performance
  marks: [
    { name: "code/server/start", startTime: 1234567890 },
    { name: "code/server/ready", startTime: 1234567900 }
  ],
  
  // Configuration
  useHostProxy: false,
  
  // User profiles
  profiles: {
    home: URI.file("~/.vscode-server/data/User/profiles"),
    all: [
      { id: "default", name: "Default", ... },
      { id: "work", name: "Work", ... }
    ]
  },
  
  // Compatibility
  isUnsupportedGlibc: false  // Linux only
}
```

### `getExtensionHostExitInfo`

Returns exit information for extension host process.

**Request**:
```typescript
{
  channelName: "remoteextensionsenvironment",
  methodName: "getExtensionHostExitInfo",
  args: [{
    reconnectionToken: "token-123"
  }]
}
```

**Response**:
```typescript
{
  code: 0,      // Exit code
  signal: null  // Signal name if killed
}
```

### `getDiagnosticInfo`

Returns diagnostic information for troubleshooting.

**Request**:
```typescript
{
  channelName: "remoteextensionsenvironment",
  methodName: "getDiagnosticInfo",
  args: [{
    includeProcesses: true,
    folders: [URI.parse("file:///workspace")]
  }]
}
```

**Response**:
```typescript
{
  machineInfo: {
    os: "Linux 5.15.0",
    cpus: "Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz (12 x 2600)",
    memory: "16.00GB (8.50GB free)",
    vmHint: "0%",
    linuxEnv: {
      desktopSession: "ubuntu",
      xdgCurrentDesktop: "ubuntu:GNOME",
      xdgSessionDesktop: "ubuntu",
      xdgSessionType: "x11"
    }
  },
  
  processes: {
    name: "node",
    pid: 12345,
    ppid: 1,
    cmd: "/path/to/node server-main.js",
    children: [
      { name: "node", pid: 12346, cmd: "extension-host" },
      { name: "bash", pid: 12347, cmd: "/bin/bash" }
    ]
  },
  
  workspaceMetadata: {
    "my-project": {
      fileCount: 1234,
      maxFilesReached: false,
      launchConfigFiles: [".vscode/launch.json"],
      configFiles: ["package.json", "tsconfig.json"]
    }
  }
}
```

## Connection Flow

### 1. Client Connects

```
Client establishes WebSocket connection
    ↓
Handshake (auth → sign → connectionType → ok)
    ↓
Client requests environment data
```

### 2. Environment Data Request

```typescript
// Client sends getEnvironmentData
const env = await channelClient.call(
  "remoteextensionsenvironment",
  "getEnvironmentData",
  { remoteAuthority: "ssh-remote+hostname" }
);
```

### 3. Server Responds

```typescript
// Server gathers environment info
const environmentData = {
  pid: process.pid,
  appRoot: URI.file(this._environmentService.appRoot),
  os: platform.OS,
  arch: process.arch,
  // ... all other fields
};

// Transform URIs for remote authority
return transformOutgoingURIs(environmentData, uriTransformer);
```

### 4. Client Uses Environment

```typescript
// Client stores environment
this._remoteAgentEnvironment = env;

// Uses paths for:
// - Extension installation (globalStorageHome)
// - Workspace storage (workspaceStorageHome)
// - Logs (logsPath, extensionHostLogsPath)
// - Settings sync (settingsPath)
```

## URI Transformation

All paths transformed to `vscode-remote://` URIs.

### Server → Client

```typescript
// Server path
appRoot: URI.file("/home/user/.vscode-server")

// Transformed for client
appRoot: URI.parse("vscode-remote://ssh-remote+hostname/home/user/.vscode-server")
```

### Client → Server

```typescript
// Client URI
vscode-remote://ssh-remote+hostname/workspace/file.txt

// Transformed for server
file:///workspace/file.txt
```

## User Data Profiles

Multiple isolated environments per user.

### Profile Structure

```
~/.vscode-server/data/User/profiles/
├── default/
│   ├── globalStorage/
│   ├── settings.json
│   └── keybindings.json
└── work/
    ├── globalStorage/
    ├── settings.json
    └── keybindings.json
```

### Profile Creation

```typescript
// Client requests profile
await getEnvironmentData({ profile: "work" });

// Server creates if doesn't exist
if (!profiles.some(p => p.id === "work")) {
  await createProfile("work", "Work");
}
```

## Performance Marks

Server tracks performance milestones.

### Common Marks

```typescript
marks: [
  { name: "code/server/start", startTime: 1234567890 },
  { name: "code/server/willLoadExtensions", startTime: 1234567895 },
  { name: "code/server/didLoadExtensions", startTime: 1234567900 },
  { name: "code/server/ready", startTime: 1234567905 }
]
```

Used by client to:
- Measure connection time
- Diagnose slow startups
- Track extension loading

## Diagnostic Information

### Machine Info

```typescript
machineInfo: {
  os: "Linux 5.15.0-91-generic",
  cpus: "Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz (12 x 2600)",
  memory: "16.00GB (8.50GB free)",
  vmHint: "0%",  // Likelihood running in VM
  
  // Linux-specific
  linuxEnv: {
    desktopSession: "ubuntu",
    xdgCurrentDesktop: "ubuntu:GNOME",
    xdgSessionDesktop: "ubuntu",
    xdgSessionType: "x11"
  }
}
```

### Process Tree

```typescript
processes: {
  name: "node",
  pid: 12345,
  ppid: 1,
  cmd: "/path/to/node server-main.js --port=8080",
  load: 5.2,  // CPU %
  mem: 2.1,   // Memory %
  
  children: [
    {
      name: "node",
      pid: 12346,
      cmd: "extension-host",
      load: 3.1,
      mem: 1.5,
      children: []
    },
    {
      name: "bash",
      pid: 12347,
      cmd: "/bin/bash",
      load: 0.1,
      mem: 0.05,
      children: []
    }
  ]
}
```

### Workspace Stats

```typescript
workspaceMetadata: {
  "my-project": {
    fileCount: 1234,
    maxFilesReached: false,  // Hit 20k file limit
    
    // Config files found
    launchConfigFiles: [".vscode/launch.json"],
    configFiles: [
      "package.json",
      "tsconfig.json",
      ".eslintrc.json",
      "webpack.config.js"
    ]
  }
}
```

## GLIBC Compatibility Check

Linux servers check GLIBC version.

```typescript
// Check GLIBC version
const glibcVersion = process.glibcVersion;  // "2.31"
const minorVersion = parseInt(glibcVersion.split('.')[1]);  // 31

// Flag if too old (< 2.28)
isUnsupportedGlibc = (minorVersion <= 27);
```

**Why**: VSCode server requires GLIBC 2.28+ for Node.js compatibility.

**Client behavior**: Shows warning if `isUnsupportedGlibc: true`.

## Connection Token

Secure token for reconnection.

```typescript
connectionToken: "abc123def456..."  // Random token

// Used for:
// - Reconnection authentication
// - Extension host identification
// - Terminal process ownership
```

**Security**: Token rotates on each connection, never reused.

## Use Cases

### Extension Installation

```typescript
// Extension installs to globalStorageHome
const extensionPath = joinPath(
  env.globalStorageHome,
  "publisher.extension-name"
);
```

### Workspace Storage

```typescript
// Workspace-specific data
const workspaceStoragePath = joinPath(
  env.workspaceStorageHome,
  workspaceId
);
```

### Logging

```typescript
// Extension host logs
const logPath = env.extensionHostLogsPath;

// Server logs
const serverLogPath = env.logsPath;
```

### Settings Sync

```typescript
// User settings
const settingsPath = env.settingsPath;

// Read/write settings
await fs.readFile(settingsPath);
await fs.writeFile(settingsPath, newSettings);
```

## Code References

### Server
- `src/vs/server/node/remoteAgentEnvironmentImpl.ts` - Environment channel
- `src/vs/server/node/serverEnvironmentService.ts` - Environment service
- `src/vs/platform/diagnostics/node/diagnosticsService.ts` - Diagnostics

### Common
- `src/vs/workbench/services/remote/common/remoteAgentEnvironmentChannel.ts` - Types
- `src/vs/platform/userDataProfile/common/userDataProfile.ts` - Profiles
- `src/vs/base/common/uriTransformer.ts` - URI transformation
