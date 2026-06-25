# Contract: Cache Settings

Four boolean settings registered in the existing registry (`src/lib/settings/index.ts`) under a new
"Caching" section. Persisted in `settings.json`; reactive reads via `settings.subscribe(key)`; the
registry-driven `SettingsDialog` auto-renders boolean descriptors (no new UI component).

## Registration

```ts
settings.registerSection({id: 'caching', label: 'Caching', order: 3});

settings.register('caching', {key: 'cache.photo', type: 'boolean', default: false,
    label: 'Photo caching', description: 'Show the viewed photo via a cached thumbnail (off = original).'});
settings.register('caching', {key: 'cache.smallThumbnails', type: 'boolean', default: false,
    label: 'Cache small thumbnails', description: 'Show list previews via cached small thumbnails (off = original).'});
settings.register('caching', {key: 'cache.lazy', type: 'boolean', default: false,
    label: 'Lazy caching', description: 'Generate thumbnails on display instead of at folder open.'});
settings.register('caching', {key: 'cache.currentFolderOnly', type: 'boolean', default: true,
    label: 'Current folder only', description: 'Cache only the current folder; do not recurse into subfolders.'});
```

## Reads (frontend)

```ts
const photoCaching = settings.subscribe<boolean>('cache.photo');
const smallCaching = settings.subscribe<boolean>('cache.smallThumbnails');
const lazy = settings.subscribe<boolean>('cache.lazy');
const currentFolderOnly = settings.subscribe<boolean>('cache.currentFolderOnly');
// use inside $derived: const wantHigh = $derived(photoCaching());
```

## Defaults & persistence

| Key | Default |
|-----|---------|
| `cache.photo` | `false` |
| `cache.smallThumbnails` | `false` |
| `cache.lazy` | `false` |
| `cache.currentFolderOnly` | `true` |

Persist immediately on change (the registry auto-saves) and take effect without restart (FR-003);
existing cache files are never deleted on toggle change (FR-009).
