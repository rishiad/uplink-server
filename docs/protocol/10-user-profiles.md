# User Data Profiles

Isolated user environments for settings, extensions, and state.

## IPC Channel: `userDataProfiles`

**Server**: `RemoteUserDataProfilesServiceChannel`  
**Location**: `sidecar/src/vs/platform/userDataProfile/common/userDataProfileIpc.ts`

## Profile Structure

```
~/.vscode-server/data/User/
├── profiles/
│   ├── default/
│   │   ├── settings.json
│   │   ├── keybindings.json
│   │   ├── snippets/
│   │   ├── globalStorage/
│   │   └── extensions.json
│   └── work/
│       ├── settings.json
│       ├── keybindings.json
│       ├── snippets/
│       ├── globalStorage/
│       └── extensions.json
└── globalStorage/  (shared)
```

## Operations

### `createProfile(name, options)`

Create new profile.

```typescript
await channel.call('createProfile', ['work', {
  name: 'Work',
  icon: 'briefcase',
  useDefaultFlags: {
    settings: false,
    keybindings: true,
    extensions: false,
    snippets: true
  }
}]);
```

**Options**:
- `name`: Display name
- `icon`: Profile icon
- `useDefaultFlags`: Inherit from default profile

### `removeProfile(profile)`

Delete profile.

```typescript
await channel.call('removeProfile', [{ id: 'work' }]);
```

### `updateProfile(profile, options)`

Update profile settings.

```typescript
await channel.call('updateProfile', [
  { id: 'work' },
  { name: 'Work Projects', icon: 'folder' }
]);
```

### `getProfiles()`

List all profiles.

```typescript
const profiles = await channel.call('getProfiles');
// Returns:
[
  { id: 'default', name: 'Default', isDefault: true },
  { id: 'work', name: 'Work', isDefault: false }
]
```

### `getDefaultProfile()`

Get default profile.

```typescript
const profile = await channel.call('getDefaultProfile');
```

## Events

### `onDidChangeProfiles`

Profile list changed.

```typescript
channel.listen('onDidChangeProfiles').subscribe(({ added, removed, updated }) => {
  // Update UI
});
```

## Profile Selection

### Connection-Time Selection

```typescript
// Client requests specific profile
const env = await channel.call('getEnvironmentData', {
  remoteAuthority: 'ssh-remote+host',
  profile: 'work'  // Profile ID
});
```

### Server Creates Missing Profile

```typescript
// Server auto-creates if doesn't exist
if (!profiles.some(p => p.id === profileId)) {
  await userDataProfilesService.createProfile(profileId, profileId);
}
```

## Profile Data

### Per-Profile Data

| Data | Location | Isolated |
|------|----------|----------|
| Settings | `profiles/<id>/settings.json` | ✅ |
| Keybindings | `profiles/<id>/keybindings.json` | ✅ |
| Snippets | `profiles/<id>/snippets/` | ✅ |
| Extensions | `profiles/<id>/extensions.json` | ✅ |
| Global Storage | `profiles/<id>/globalStorage/` | ✅ |

### Shared Data

| Data | Location |
|------|----------|
| Machine Settings | `data/Machine/settings.json` |
| Logs | `data/logs/` |
| Cache | `data/CachedData/` |

## Profile Interface

```typescript
interface IUserDataProfile {
  id: string;
  name: string;
  isDefault: boolean;
  icon?: string;
  
  // Paths
  globalStorageHome: URI;
  settingsResource: URI;
  keybindingsResource: URI;
  snippetsHome: URI;
  extensionsResource: URI;
  
  // Inheritance
  useDefaultFlags?: {
    settings?: boolean;
    keybindings?: boolean;
    extensions?: boolean;
    snippets?: boolean;
  };
}
```

## Environment Data with Profiles

```typescript
// getEnvironmentData response includes profile info
{
  // ... other fields
  
  profiles: {
    home: URI.file("~/.vscode-server/data/User/profiles"),
    all: [
      {
        id: "default",
        name: "Default",
        isDefault: true,
        globalStorageHome: URI.file("~/.vscode-server/data/User/globalStorage"),
        settingsResource: URI.file("~/.vscode-server/data/User/settings.json")
      },
      {
        id: "work",
        name: "Work",
        isDefault: false,
        globalStorageHome: URI.file("~/.vscode-server/data/User/profiles/work/globalStorage"),
        settingsResource: URI.file("~/.vscode-server/data/User/profiles/work/settings.json")
      }
    ]
  }
}
```

## Use Cases

### Separate Work/Personal

```
Profile: Personal
├── Theme: Dark+
├── Extensions: Games, Social
└── Settings: Relaxed linting

Profile: Work
├── Theme: Light
├── Extensions: Enterprise tools
└── Settings: Strict linting
```

### Project-Specific

```
Profile: Python-ML
├── Extensions: Python, Jupyter, Pylance
└── Settings: Python-specific

Profile: Web-Dev
├── Extensions: ESLint, Prettier, React
└── Settings: JavaScript-specific
```

## Code References

- `src/vs/platform/userDataProfile/common/userDataProfileIpc.ts` - IPC channel
- `src/vs/platform/userDataProfile/common/userDataProfile.ts` - Interfaces
- `src/vs/platform/userDataProfile/node/userDataProfile.ts` - Server implementation
- `src/vs/server/node/serverServices.ts` - Channel registration
