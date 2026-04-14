# Collections & Folders

Collections organize your API requests into named groups with nested folder support. All collections are stored as JSON files on disk, making them naturally Git-friendly.

## Creating a Collection

1. Open the **Collections** tab in the sidebar.
2. Click the **+** button.
3. Type a name and press **Enter**.

Nexus creates a directory under `~/.nexus/collections/` with the collection metadata and subdirectories for requests and folders.

## Collection Structure on Disk

```
~/.nexus/collections/
└── my-api/
    ├── collection.json       # Collection metadata
    ├── requests/
    │   ├── get-users.json    # Individual request files
    │   └── create-user.json
    └── folders/
        └── admin/
            ├── folder.json   # Folder metadata
            └── ...           # Nested requests and folders
```

Each collection directory contains:

- **`collection.json`** — Name, description, version, variables, auth config, scripts, and item ordering.
- **`requests/`** — One JSON file per request, named with a slugified version of the request name.
- **`folders/`** — Subdirectories for nested folders, each with their own `folder.json` and nested items.

## Collection Properties

| Property | Description |
|---|---|
| `name` | Display name of the collection |
| `description` | Optional description |
| `version` | Version string (e.g., "1.0.0") |
| `variables` | Collection-scoped variables (see [Environments](./environments.md)) |
| `auth` | Collection-level auth config inherited by all requests |
| `preRequestScript` | Script that runs before every request in the collection |
| `postResponseScript` | Script that runs after every request in the collection |
| `itemOrder` | Ordered list of item IDs controlling display order |

## Managing Requests

### Adding a Request

- **From the sidebar**: Right-click a collection → "Add Request".
- **From the request builder**: Click **Save ▾** → select a collection → enter a name.

### Saving Changes

When a request tab is linked to a collection (has an "origin"), clicking **Save** updates the request file on disk. Unsaved changes are indicated by a blue dot (●) on the tab.

### Deleting a Request

Right-click a request in the collection tree → "Delete Request". The JSON file is removed from disk.

## Nested Folders

Folders provide nested organization within a collection. Each folder can contain requests and sub-folders at arbitrary depth.

Folders support the same inherited properties as collections:
- **Auth** — Requests in the folder inherit the folder's auth config.
- **Pre-request script** — Runs before any request in the folder.
- **Post-response script** — Runs after any request in the folder.

## Script Execution Order

When a request is inside nested folders, scripts execute in this order:

```
Collection pre-request script
  └─ Folder pre-request script (outermost first)
       └─ Request pre-request script
            └─ *** HTTP REQUEST IS SENT ***
       └─ Request post-response script
  └─ Folder post-response script (innermost first)
└─ Collection post-response script
```

## Auth Inheritance

Requests inherit authentication from their parent:

1. If a request's auth is set to **Inherit** (the default), it uses its parent folder's auth.
2. If the folder's auth is also **Inherit**, it walks up to the collection level.
3. Collection-level auth applies to all requests that don't override it.

Setting a request's auth to any specific type (Bearer, Basic, etc.) overrides inheritance.

## Renaming Collections

Right-click a collection in the sidebar → "Rename". An inline input appears where you can type the new name. This updates the `collection.json` file on disk.

## Deleting Collections

Right-click a collection → "Delete Collection". This recursively removes the entire collection directory from disk. This action cannot be undone.

## Collection Runner

Collections can be executed as automated test suites using the [Collection Runner](../automation/collection-runner.md). The runner executes requests sequentially with configurable iterations, delays, and data files.
