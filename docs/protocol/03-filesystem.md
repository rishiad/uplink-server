# Filesystem Protocol

Remote filesystem operations over IPC.

**Channel Name**: `vscode.remoteFileSystemProvider`

**Location**: `uplink-server/sidecar/src/vs/server/node/remoteFileSystemProviderServer.ts`

## Architecture

```
VSCode Client                          uplink-server
─────────────────────────────────────────────────────────

FileSystemProvider (virtual)
    ↓
IPC Channel Client
    ↓
[IPC Messages]
                                       ↓
                                  IPC Channel Server
                                       ↓
                            RemoteAgentFileSystemProviderChannel
                                       ↓
                            DiskFileSystemProvider
                                       ↓
                                  Node.js fs module
                                       ↓
                                  Actual filesystem
```

## Interface

```typescript
interface FileSystemProvider {
  // File Operations
  stat(uri: URI): Promise<FileStat>;
  readFile(uri: URI): Promise<Uint8Array>;
  writeFile(uri: URI, content: Uint8Array, options: WriteFileOptions): Promise<void>;
  delete(uri: URI, options: DeleteOptions): Promise<void>;
  rename(oldUri: URI, newUri: URI, options: RenameOptions): Promise<void>;
  copy(source: URI, destination: URI, options: CopyOptions): Promise<void>;
  
  // Directory Operations
  readDirectory(uri: URI): Promise<[string, FileType][]>;
  createDirectory(uri: URI): Promise<void>;
  
  // Watching
  watch(uri: URI, options: WatchOptions): Disposable;
  
  // Events
  onDidChangeFile: Event<FileChange[]>;
  onDidWatchError: Event<string>;
}
```

**Interface Location**: `sidecar/src/vs/platform/files/common/files.ts`

## Data Types

### FileStat

```typescript
interface FileStat {
  type: FileType;
  ctime: number;  // Creation time (ms since epoch)
  mtime: number;  // Modification time (ms since epoch)
  size: number;   // File size in bytes
}

enum FileType {
  Unknown = 0,
  File = 1,
  Directory = 2,
  SymbolicLink = 64
}
```

### FileChange

```typescript
interface FileChange {
  type: FileChangeType;
  resource: URI;
}

enum FileChangeType {
  UPDATED = 0,
  ADDED = 1,
  DELETED = 2
}
```

### Options

```typescript
interface WriteFileOptions {
  create: boolean;
  overwrite: boolean;
  unlock: boolean;
}

interface DeleteOptions {
  recursive: boolean;
  useTrash: boolean;
}

interface RenameOptions {
  overwrite: boolean;
}

interface CopyOptions {
  overwrite: boolean;
}
```

## URI Transformation

**Location**: `remoteFileSystemProviderServer.ts:transformIncoming()`

### Client → Server

```typescript
// Client URI
vscode-remote://ssh-remote+hostname/home/user/file.txt

// IURITransformer
const transformer = createURITransformer('ssh-remote+hostname');
const serverUri = transformer.transformIncoming(clientUri);

// Server URI
file:///home/user/file.txt
```

### Server → Client

```typescript
// Server URI
file:///home/user/file.txt

// IURITransformer
const clientUri = transformer.transformOutgoing(serverUri);

// Client URI
vscode-remote://ssh-remote+hostname/home/user/file.txt
```

**Transformer Location**: `sidecar/src/vs/base/common/uriTransformer.ts`

## Operations

### stat

Get file/directory metadata.

```typescript
// Client
const stat = await fs.stat(URI.parse('vscode-remote://ssh-remote+host/file.txt'));
// Returns: { type: FileType.File, ctime: 1234567890, mtime: 1234567890, size: 1024 }

// IPC Message
[RequestType.Promise, id, "vscode.remoteFileSystemProvider", "stat", 
 [{ scheme: "vscode-remote", authority: "ssh-remote+host", path: "/file.txt" }]]

// Server
const stat = await fs.promises.stat('/file.txt');
return { type: FileType.File, ctime: stat.ctimeMs, mtime: stat.mtimeMs, size: stat.size };
```

### readFile

Read entire file contents.

```typescript
// Client
const content = await fs.readFile(uri);
// Returns: Uint8Array

// IPC Message
[RequestType.Promise, id, "vscode.remoteFileSystemProvider", "readFile", [uri]]

// Server
const buffer = await fs.promises.readFile(path);
return VSBuffer.wrap(buffer);
```

**Note**: Files sent as raw bytes (VSBuffer), not base64 encoded.

### writeFile

Write entire file contents.

```typescript
// Client
await fs.writeFile(uri, content, { create: true, overwrite: true });

// IPC Message
[RequestType.Promise, id, "vscode.remoteFileSystemProvider", "writeFile",
 [uri, VSBuffer(content), { create: true, overwrite: true }]]

// Server
await fs.promises.writeFile(path, content);
```

### readDirectory

List directory contents.

```typescript
// Client
const entries = await fs.readDirectory(uri);
// Returns: [['file.txt', FileType.File], ['subdir', FileType.Directory]]

// IPC Message
[RequestType.Promise, id, "vscode.remoteFileSystemProvider", "readDirectory", [uri]]

// Server
const entries = await fs.promises.readdir(path, { withFileTypes: true });
return entries.map(e => [e.name, e.isDirectory() ? FileType.Directory : FileType.File]);
```

### createDirectory

Create directory (and parents if needed).

```typescript
// Client
await fs.createDirectory(uri);

// IPC Message
[RequestType.Promise, id, "vscode.remoteFileSystemProvider", "createDirectory", [uri]]

// Server
await fs.promises.mkdir(path, { recursive: true });
```

### delete

Delete file or directory.

```typescript
// Client
await fs.delete(uri, { recursive: true, useTrash: false });

// IPC Message
[RequestType.Promise, id, "vscode.remoteFileSystemProvider", "delete",
 [uri, { recursive: true, useTrash: false }]]

// Server
if (recursive) {
  await fs.promises.rm(path, { recursive: true });
} else {
  await fs.promises.unlink(path);
}
```

### rename

Move/rename file or directory.

```typescript
// Client
await fs.rename(oldUri, newUri, { overwrite: true });

// IPC Message
[RequestType.Promise, id, "vscode.remoteFileSystemProvider", "rename",
 [oldUri, newUri, { overwrite: true }]]

// Server
await fs.promises.rename(oldPath, newPath);
```

### copy

Copy file or directory.

```typescript
// Client
await fs.copy(sourceUri, destUri, { overwrite: true });

// IPC Message
[RequestType.Promise, id, "vscode.remoteFileSystemProvider", "copy",
 [sourceUri, destUri, { overwrite: true }]]

// Server
await fs.promises.copyFile(sourcePath, destPath);
```

## File Watching

**Location**: `remoteFileSystemProviderServer.ts:SessionFileWatcher`

### Watch Setup

```typescript
// Client subscribes to changes
const disposable = fs.onDidChangeFile(changes => {
  for (const change of changes) {
    console.log(change.type, change.resource);
  }
});

// IPC Message
[RequestType.EventListen, id, "vscode.remoteFileSystemProvider", "onDidChangeFile", null]

// Server creates watcher
const watcher = fs.watch(path, { recursive: true });
watcher.on('change', (event, filename) => {
  emitter.fire([{
    type: FileChangeType.UPDATED,
    resource: URI.file(path + '/' + filename)
  }]);
});
```

### Change Events

```typescript
// Server fires event
[ResponseType.EventFire, id, [
  { type: FileChangeType.UPDATED, resource: { scheme: "file", path: "/file.txt" } },
  { type: FileChangeType.ADDED, resource: { scheme: "file", path: "/new.txt" } }
]]

// Client receives
onDidChangeFile.fire([
  { type: FileChangeType.UPDATED, resource: URI.parse('vscode-remote://...') },
  { type: FileChangeType.ADDED, resource: URI.parse('vscode-remote://...') }
]);
```

### Watcher Options

```typescript
interface IRecursiveWatcherOptions {
  usePolling?: boolean | string[];  // Use polling instead of native watchers
  pollingInterval?: number;          // Polling interval in ms
}
```

**Configuration**: Via `--file-watcher-polling` server argument

### Debouncing

Multiple rapid changes coalesced into single event:

```typescript
// Server debounces changes
const debounced = debounce((changes: FileChange[]) => {
  emitter.fire(changes);
}, 50);  // 50ms window
```

## Caching Strategy

VSCode client caches file contents in memory.

### Cache Structure

```typescript
class FileService {
  private cache = new Map<string, {
    content: Uint8Array;
    mtime: number;
    etag: string;
  }>();
}
```

### Cache Validation

```typescript
async readFile(uri: URI): Promise<Uint8Array> {
  const cached = this.cache.get(uri.toString());
  
  if (cached) {
    // Validate with stat
    const stat = await this.stat(uri);
    if (stat.mtime === cached.mtime) {
      return cached.content;  // Cache hit
    }
  }
  
  // Cache miss - fetch from server
  const content = await this.provider.readFile(uri);
  this.cache.set(uri.toString(), { content, mtime: stat.mtime });
  return content;
}
```

### Cache Invalidation

```typescript
// On file change event
onDidChangeFile(changes => {
  for (const change of changes) {
    this.cache.delete(change.resource.toString());
    
    // Reload open editors
    const editor = this.getEditorForUri(change.resource);
    if (editor) {
      editor.reload();
    }
  }
});
```

## Performance Optimizations

### 1. Lazy Loading

Only fetch what's visible:
```typescript
// Opening folder - only list root
const entries = await fs.readDirectory(rootUri);

// User expands directory - fetch that directory
const subEntries = await fs.readDirectory(subDirUri);
```

### 2. Batch Operations

```typescript
// Concurrent stats
const stats = await Promise.all(
  uris.map(uri => fs.stat(uri))
);
```

### 3. Prefetching

```typescript
// When opening folder, prefetch common files
const prefetch = ['package.json', 'tsconfig.json', '.gitignore']
  .map(f => fs.readFile(URI.joinPath(folder, f)));
await Promise.allSettled(prefetch);
```

### 4. Streaming (Not Implemented)

Current: Read entire file into memory
Potential: Stream large files in chunks

## Error Handling

### Common Errors

```typescript
// File not found
FileSystemProviderError.FileNotFound(uri)

// File exists
FileSystemProviderError.FileExists(uri)

// No permissions
FileSystemProviderError.NoPermissions(uri)

// Is directory
FileSystemProviderError.FileIsADirectory(uri)
```

### Error Propagation

```typescript
// Server throws
throw new Error('ENOENT: no such file or directory');

// IPC serializes
[ResponseType.PromiseError, id, {
  message: 'ENOENT: no such file or directory',
  name: 'Error',
  stack: [...]
}]

// Client reconstructs
const error = new Error('ENOENT: no such file or directory');
throw error;
```

## Code References

### Server
- `sidecar/src/vs/server/node/remoteFileSystemProviderServer.ts` - Main channel
- `sidecar/src/vs/platform/files/node/diskFileSystemProvider.ts` - Disk implementation
- `sidecar/src/vs/platform/files/node/diskFileSystemProviderServer.ts` - Base channel
- `sidecar/src/vs/platform/files/common/watcher.ts` - File watching

### Client (Built-in VSCode)
- File service registration
- Cache management
- Explorer UI integration

### Common
- `sidecar/src/vs/platform/files/common/files.ts` - Interfaces
- `sidecar/src/vs/base/common/uri.ts` - URI handling
- `sidecar/src/vs/base/common/uriTransformer.ts` - URI transformation
