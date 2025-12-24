# Uplink Server
Custom VSCode server implementation for Uplink remote development extension built from the source tree of [openvscode-server](https://github.com/gitpod-io/openvscode-server). The protocol docs outlining the communication between uplink server, uplink extension and vscode client are under `docs/protocol` 

## Architecture
The server consists of:
- **VSCode Server**: Modified openvscode server with lowmem build tasks (`./vscode-server/`)
  - Node.js/TypeScript codebase built with npm/gulp
- **Rust Launcher**: Native binary that wraps the Node.js server 
  - Provides `uplink-server` executable
  - Handles Node.js process spawning and argument forwarding
  - Supports GLIBC patching for compatibility
- **Packager**: Rust utility to bundle vscode-server + launcher into distributable tarball 
  - Combines built vscode-server with launcher binary
  - Creates `.tar.gz` archives for distribution
- **Build System**: Docker-based multi-variant build system
  - Builds both Node.js vscode-server and Rust launcher
  - Packages complete server distributions

## Building
```bash
./build-servers.sh
```

## Output Structure

Builds are created in `./dist/` directory:

```
dist/
├── uplink-server-x64-<version>.tar.gz
└── uplink-server-arm64-<version>.tar.gz
```

## Development

### Modifying the VSCode Server

The VSCode server source is in `./vscode-server/`. To make changes:

1. Modify source in `./vscode-server/`
2. Test locally with `npm ci && npm run gulp vscode-server-linux-x64-lowmem`
