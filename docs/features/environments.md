# Environments & Variables

Environments let you define sets of variables that can be swapped to test the same requests against different servers (development, staging, production) without changing the requests themselves.

## Creating an Environment

1. Open the **Envs** tab in the sidebar.
2. Click **+ New Environment**.
3. Enter a name (e.g., "Development").
4. Add variables with key-value pairs.
5. Click **Save**.

Environments are stored as individual JSON files in `~/.nexus/environments/`.

## Using Variables

Reference variables anywhere in requests using double-curly-brace syntax:

```
{{variable_name}}
```

Variables can be used in:
- URLs: `{{base_url}}/api/users`
- Headers: `Authorization: Bearer {{token}}`
- Query parameters: key or value fields
- Request body: anywhere in the raw body text
- Authentication fields: token, username, password, etc.

## Variable Scopes

Nexus resolves variables through a layered scope system. When a `{{variable}}` is referenced, scopes are searched in this order (first match wins):

| Priority | Scope | Description |
|---|---|---|
| 1 (highest) | **Local** | Temporary variables set during script execution. Discarded after the request. |
| 2 | **Environment** | Variables from the active environment file. |
| 3 | **Collection** | Variables defined in `collection.json`. |
| 4 (lowest) | **Global** | Global variables shared across all requests. |

If a variable is not found in any scope, the `{{variable}}` placeholder is left as-is in the output.

## Activating an Environment

Select an environment from the dropdown in the toolbar (center). Only one environment can be active at a time. Select "No Environment" to deactivate all environments.

## Variable Properties

Each variable has:

| Property | Description |
|---|---|
| `key` | Variable name used in `{{key}}` references |
| `value` | The value to substitute |
| `type` | `string` (default) or `secret` |
| `enabled` | Toggle the variable on/off without deleting it |
| `description` | Optional documentation |

### Secret Variables

Variables with type `secret` have their values masked in the UI (shown as password dots). Secrets are:
- Hidden in the environment editor by default (toggle visibility with the eye icon).
- Exported as empty strings when exporting environments in legacy format.
- Never displayed in plain text in the console or logs.

## Setting Variables from Scripts

Pre-request and post-response scripts can read and write variables at runtime:

```javascript
// Read from a specific scope
const token = nx.environment.get("auth_token");

// Write to a specific scope
nx.environment.set("auth_token", "new-value");

// Read from the highest-priority scope
const baseUrl = nx.variables.get("base_url");

// Resolve {{variables}} in a string
const resolved = nx.variables.replaceIn("{{base_url}}/users/{{user_id}}");
```

See the [nx API Reference](../scripting/nx-api-reference.md) for the complete scripting API.

## Dynamic Variables

Nexus includes 119 built-in dynamic variables that generate random data on each request. Use them with the `$` prefix:

```
{{$randomEmail}}       → alice42@example.com
{{$timestamp}}         → 1713100800
{{$randomUUID}}        → a1b2c3d4-e5f6-7890-abcd-ef1234567890
{{$randomFullName}}    → Jane Smith
```

See the full list in [Dynamic Variables Reference](../reference/dynamic-variables.md).

## Environment Storage

Each environment is stored as a standalone JSON file:

```json
// ~/.nexus/environments/env_abc123.json
{
  "id": "env_abc123",
  "name": "Development",
  "variables": [
    {
      "key": "base_url",
      "value": "http://localhost:3000",
      "type": "string",
      "enabled": true,
      "description": "API base URL"
    },
    {
      "key": "api_key",
      "value": "sk-dev-secret-key",
      "type": "secret",
      "enabled": true
    }
  ]
}
```

## Managing Environments

- **Edit**: Click an environment name in the sidebar to open the variable editor.
- **Delete**: Click the trash icon next to an environment (requires confirmation).
- **Duplicate**: Not yet supported — copy the JSON file manually on disk.
