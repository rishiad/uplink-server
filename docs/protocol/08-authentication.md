# Authentication Flow

WebSocket connection handshake between client and server.

## Handshake Sequence

```
Client                                    Server
   │                                         │
   │────────WebSocket Upgrade ──────────────▶│
   │        ?reconnectionToken=xxx           │
   │         reconnection=false              │
   │                                         │
   │◀──────── 101 Switching Protocols ───────│
   │                                         │
   │──────── { type: "auth", auth, data } ──▶│
   │                                         │
   │◀────{ type: "sign", data, signedData } ─│
   │                                         │
   │────────{ type: "connectionType", ... } ▶│
   │                                         │
   │◀──────── { type: "ok" } ────────────────│
   │                                         │
   │         Connection Established          │
```

## Step 1: WebSocket Upgrade

Client initiates WebSocket connection with query parameters:

```
GET /?reconnectionToken=abc123&reconnection=false&skipWebSocketFrames=false
Connection: Upgrade
Upgrade: websocket
```

**Parameters**:
- `reconnectionToken`: UUID for session identification
- `reconnection`: `true` if reconnecting to existing session
- `skipWebSocketFrames`: `true` for raw TCP (no WebSocket framing)

## Step 2: Auth Message

Client sends authentication message:

```typescript
{
  type: "auth",
  auth: "connection-token-value",  // Server's connection token
  data: "random-challenge-data"    // Random data for signing
}
```

**Server validates**:
```typescript
if (connectionToken.type === ServerConnectionTokenType.Mandatory) {
  if (!connectionToken.validate(msg.auth)) {
    return rejectConnection("auth mismatch");
  }
}
```

## Step 3: Sign Response

Server responds with signed challenge:

```typescript
{
  type: "sign",
  data: "validator-challenge",     // Challenge for client to sign
  signedData: "signed-client-data" // Server's signature of client's data
}
```

**Signing** (if vsda module available):
```typescript
const signer = new vsda.signer();
signedData = signer.sign(clientData);

const validator = new vsda.validator();
someText = validator.createNewMessage(someText);
```

## Step 4: Connection Type

Client sends connection type and signed response:

```typescript
{
  type: "connectionType",
  desiredConnectionType: 1,  // 1=Management, 2=ExtensionHost, 3=Tunnel
  signedData: "signed-challenge",
  commit: "abc123def456",    // VSCode commit hash
  args: { ... }              // Connection-specific args
}
```

**Connection Types**:
```typescript
enum ConnectionType {
  Management = 1,      // Main IPC channel
  ExtensionHost = 2,   // Extension host process
  Tunnel = 3           // Port forwarding
}
```

**Server validates**:
```typescript
// Version check
if (rendererCommit !== myCommit) {
  return rejectConnection("version mismatch");
}

// Signature validation
if (validator) {
  valid = validator.validate(msg.signedData) === 'ok';
}
```

## Step 5: OK Response

Server confirms connection:

```typescript
{ type: "ok" }

// For ExtensionHost, may include debug port:
{ debugPort: 9229 }
```

## Connection Token

### Token Types

```typescript
enum ServerConnectionTokenType {
  None = 0,       // No authentication required
  Optional = 1,   // Token accepted but not required
  Mandatory = 2   // Token required
}
```

### Token Generation

```typescript
// Server startup
const connectionToken = await determineServerConnectionToken(args);

// From CLI args
--connection-token <token>
--connection-token-file <path>

// Or generated
connectionToken = generateUuid();
```

### Token Validation

```typescript
class ServerConnectionToken {
  validate(token: string): boolean {
    if (this.type === ServerConnectionTokenType.None) {
      return true;
    }
    return this.value === token;
  }
}
```

## VSDA Module

Optional native module for cryptographic signing.

```typescript
// Check if available
const hasVSDA = fs.existsSync('node_modules/vsda');

// Signer - signs data
const signer = new vsda.signer();
const signature = signer.sign(data);

// Validator - creates and validates challenges
const validator = new vsda.validator();
const challenge = validator.createNewMessage(text);
const result = validator.validate(signedData);  // 'ok' or 'error'
```

**Without VSDA**: Server accepts connection token as signature.

## Error Handling

### Rejection Messages

```typescript
{
  type: "error",
  reason: "Unauthorized client refused: auth mismatch"
}
```

### Common Errors

| Error | Cause |
|-------|-------|
| `auth mismatch` | Invalid connection token |
| `version mismatch` | Client/server commit mismatch |
| `Unauthorized client refused` | Invalid signature |
| `Unknown reconnection token` | Invalid reconnection attempt |
| `Duplicate reconnection token` | Token already in use |

## HTTP Requests

HTTP requests also require token validation:

```typescript
// Token in query string
GET /vscode-remote-resource?tkn=<token>&path=/file.txt

// Validation
function httpRequestHasValidConnectionToken(token, req, parsedUrl) {
  const queryToken = parsedUrl.query[connectionTokenQueryName];
  return token.validate(queryToken);
}
```

## Security Considerations

1. **Token secrecy**: Connection token should be kept secret
2. **HTTPS**: Use HTTPS in production for token protection
3. **Token rotation**: Tokens are per-session, not persistent
4. **Version matching**: Prevents protocol incompatibilities
5. **Signature validation**: Prevents replay attacks (with vsda)

## Code References

- `src/vs/server/node/remoteExtensionHostAgentServer.ts` - Handshake handling
- `src/vs/server/node/serverConnectionToken.ts` - Token management
- `src/vs/platform/remote/common/remoteAgentConnection.ts` - Connection types
