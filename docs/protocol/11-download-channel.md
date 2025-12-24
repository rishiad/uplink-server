# Download Channel

Reverse channel where server requests downloads from client.

## IPC Channel: `download`

**Direction**: Server → Client (reverse of normal)  
**Client**: `DownloadServiceChannelClient`  
**Location**: `sidecar/src/vs/platform/download/common/downloadIpc.ts`

## Architecture

```
┌─────────────────────────────────────────────┐
│ Remote Server                               │
│  ├─ Needs to download file                  │
│  └─ Requests via download channel           │
└─────────────────────────────────────────────┘
                    │
                    │ IPC Request
                    ▼
┌─────────────────────────────────────────────┐
│ VSCode Client                               │
│  ├─ Has network access                      │
│  ├─ Downloads file                          │
│  └─ Streams to server                       │
└─────────────────────────────────────────────┘
```

## Why Reverse Direction?

Server may not have direct internet access:
- Behind corporate firewall
- Air-gapped environment
- Restricted network policies

Client typically has:
- Direct internet access
- Proxy configuration
- Authentication credentials

## Operations

### `download(uri, target, options)`

Request client to download file.

```typescript
// Server requests download
const downloadService = accessor.get(IDownloadService);
await downloadService.download(
  URI.parse('https://marketplace.visualstudio.com/extension.vsix'),
  URI.file('/tmp/extension.vsix')
);
```

**Flow**:
```
Server: "Please download https://... to /tmp/..."
    ↓
Client: Downloads file from URL
    ↓
Client: Streams content back to server
    ↓
Server: Writes to target path
```

## Channel Setup

### Server Side

```typescript
// Server gets channel from client
const router = new StaticRouter(ctx => ctx.clientId === 'renderer');
const downloadChannel = socketServer.getChannel('download', router);

// Create service from channel
const downloadService = new DownloadServiceChannelClient(
  downloadChannel,
  () => getUriTransformer('renderer')
);
```

### Client Side

```typescript
// Client registers download channel
const downloadService = accessor.get(IDownloadService);
const channel = new DownloadServiceChannel(downloadService);
mainProcessService.registerChannel('download', channel);
```

## Use Cases

### Extension Installation

```typescript
// Server needs extension from marketplace
await downloadService.download(
  URI.parse('https://marketplace.visualstudio.com/_apis/public/gallery/publishers/ms-python/vsextensions/python/2024.1.0/vspackage'),
  URI.file('/tmp/ms-python.python-2024.1.0.vsix')
);
```

### Language Server Download

```typescript
// Server needs language server binary
await downloadService.download(
  URI.parse('https://github.com/rust-lang/rust-analyzer/releases/download/2024-01-15/rust-analyzer-x86_64-unknown-linux-gnu.gz'),
  URI.file('/tmp/rust-analyzer.gz')
);
```

## Events

### `onDidDownload`

Download completed.

```typescript
channel.listen('onDidDownload').subscribe(({ uri, target }) => {
  console.log(`Downloaded ${uri} to ${target}`);
});
```

## URI Transformation

URIs transformed between client and server:

```typescript
// Server path
target: URI.file('/home/user/.vscode-server/extensions/ext.vsix')

// Transformed for client
target: URI.parse('vscode-remote://ssh-remote+host/home/user/.vscode-server/extensions/ext.vsix')

// Client writes via filesystem channel
await remoteFileSystem.writeFile(target, downloadedContent);
```

## Error Handling

```typescript
try {
  await downloadService.download(uri, target);
} catch (error) {
  if (error.code === 'ENOTFOUND') {
    // Network unreachable
  } else if (error.code === 'EACCES') {
    // Permission denied on target
  } else if (error.statusCode === 404) {
    // Resource not found
  }
}
```

## Code References

- `src/vs/platform/download/common/downloadIpc.ts` - Channel implementation
- `src/vs/platform/download/common/download.ts` - Service interface
- `src/vs/server/node/serverServices.ts` - Channel setup
