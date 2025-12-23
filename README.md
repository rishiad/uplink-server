# Uplink Server
Custom VSCode server implementation for Uplink remote development extension built from the source tree of [openvscode-server](https://github.com/gitpod-io/openvscode-server). 

## Architecture
The server consists of:
- **Custom VSCode Server Sidecar**: Modified openvscode server with lowmem build tasks (`./sidecar/`)
  - Node.js/TypeScript codebase built with npm/gulp
- **Rust Launcher**: Native binary that wraps the Node.js server 
  - Provides `uplink-server` executable
  - Handles Node.js process spawning and argument forwarding
  - Supports GLIBC patching for compatibility
- **Packager**: Rust utility to bundle sidecar + launcher into distributable tarball 
  - Combines built sidecar with launcher binary
  - Creates `.tar.gz` archives for distribution
- **Build System**: Docker-based multi-variant build system
  - Builds both Node.js sidecar and Rust launcher
  - Packages complete server distributions

## Building
```bash
./build-servers.sh
```

## Output Structure

Builds are created in `./dist/` directory:

```
dist/
├── server-linux-x64-glibc-<version>.tar.gz
└── server-linux-arm64-glibc-<version>.tar.gz
```

## Development

### Modifying the Sidecar

The custom VSCode sidecar is in `./sidecar/`. To make changes:

1. Modify source in `./sidecar/`
2. Test locally with `npm ci && npm run gulp vscode-server-linux-x64-lowmem`
