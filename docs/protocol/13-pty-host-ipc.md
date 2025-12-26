# PTY Host IPC Protocol

## Overview

The PTY host is a separate process spawned by VS Code server to manage terminal sessions. It communicates via stdin/stdout using a custom binary IPC protocol.

## Transport

- Messages are base64-encoded binary buffers
- Sent over stdin/stdout of child process
- Bidirectional: requests from server, responses + events from PTY host

## Binary Serialization

Each message = `serialize(header) + serialize(body)`

### Data Types

| Type | ID | Format |
|------|-----|--------|
| Undefined | 0 | (none) |
| String | 1 | VQL(len) + UTF-8 bytes |
| Buffer | 2 | VQL(len) + raw bytes |
| VSBuffer | 3 | VQL(len) + raw bytes |
| Array | 4 | VQL(len) + elements |
| Object | 5 | VQL(len) + JSON string |
| Int | 6 | VQL(value) |

### VQL (Variable-Length Quantity)

7 bits per byte, MSB=1 means continue:
```
0-127:     [0xxxxxxx]
128-16383: [1xxxxxxx] [0xxxxxxx]
```

## Message Types

### Requests (Server → PTY Host)

| Type | ID | Header |
|------|-----|--------|
| Promise | 100 | `[100, reqId, channel, method]` + args |
| PromiseCancel | 101 | `[101, reqId]` |
| EventListen | 102 | `[102, reqId, channel, event]` + arg |
| EventDispose | 103 | `[103, reqId]` |

### Responses (PTY Host → Server)

| Type | ID | Header |
|------|-----|--------|
| Initialize | 200 | `[200]` |
| PromiseSuccess | 201 | `[201, reqId]` + data |
| PromiseError | 202 | `[202, reqId]` + `{message, name, stack}` |
| PromiseErrorObj | 203 | `[203, reqId]` + data |
| EventFire | 204 | `[204, reqId]` + data |

## Channels

### `ptyHost`

#### Methods

| Method | Args | Returns |
|--------|------|---------|
| `createProcess` | (shellLaunchConfig, cwd, cols, rows, unicodeVersion, env, executableEnv, options, shouldPersist, workspaceId, workspaceName) | `number` |
| `start` | (id) | `undefined \| {message} \| {injectedArgs}` |
| `input` | (id, data) | `void` |
| `resize` | (id, cols, rows) | `void` |
| `shutdown` | (id, immediate) | `void` |
| `shutdownAll` | () | `void` |
| `getLatency` | () | `[]` |
| `listProcesses` | () | `IProcessDetails[]` |
| `getDefaultSystemShell` | (osOverride?) | `string` |
| `getEnvironment` | () | `Record<string,string>` |
| `getCwd` | (id) | `string` |
| `getInitialCwd` | (id) | `string` |
| `acknowledgeDataEvent` | (id, charCount) | `void` |
| `setUnicodeVersion` | (id, version) | `void` |
| `orphanQuestionReply` | (id) | `void` |

#### Events

| Event | Payload |
|-------|---------|
| `onProcessData` | `{id, event: string \| {data, trackCommit}}` |
| `onProcessReady` | `{id, event: {pid, cwd, windowsPty?}}` |
| `onProcessExit` | `{id, event: number \| undefined}` |
| `onDidChangeProperty` | `{id, property: {type, value}}` |
| `onProcessReplay` | `{id, event: {events, commands}}` |
| `onProcessOrphanQuestion` | `{id}` |

### `heartbeat`

#### Events

| Event | Payload |
|-------|---------|
| `onBeat` | `void` |

Fire every ~5 seconds.

## Startup Sequence

1. Server spawns PTY host process
2. PTY host sends `[200]` (Initialize)
3. Server subscribes to events via `EventListen`
4. Server calls `createProcess` → gets ID
5. Server calls `start(id)` → shell spawns
6. PTY host fires `onProcessReady`
7. Data flows: `input()` ↔ `onProcessData`
8. Exit: `shutdown()` or `onProcessExit`

## Flow Control

| Constant | Value |
|----------|-------|
| HighWatermark | 100,000 chars |
| LowWatermark | 5,000 chars |
| AckSize | 5,000 chars |

Pause PTY at high watermark, resume at low watermark.

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `VSCODE_RECONNECT_GRACE_TIME` | Reconnection timeout (ms) |
| `VSCODE_RECONNECT_SHORT_GRACE_TIME` | Short timeout (ms) |
| `VSCODE_RECONNECT_SCROLLBACK` | Buffer size |
