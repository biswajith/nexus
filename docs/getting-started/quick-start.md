# Quick Start Guide

This guide walks you through sending your first API request, saving it to a collection, and using environments.

## 1. Send Your First Request

1. Launch Nexus. A new request tab opens automatically.
2. Select **GET** from the method dropdown.
3. Type a URL in the URL bar, for example: `https://jsonplaceholder.typicode.com/posts/1`
4. Click **Send** (or press `Cmd+Enter` / `Ctrl+Enter`).

The response appears in the bottom pane with the status code, response time, size, and formatted JSON body.

## 2. Add Headers and Query Parameters

Switch between the **Params**, **Headers**, **Body**, **Auth**, and **Scripts** tabs below the URL bar.

**Query Parameters:**

1. Click the **Params** tab.
2. Add a key-value pair, e.g., `userId` = `1`.
3. Enable/disable individual params with the checkbox.
4. The URL bar updates automatically to reflect the query string.

**Headers:**

1. Click the **Headers** tab.
2. Add custom headers like `Accept: application/json`.
3. Each header has a key, value, and optional description field.

## 3. Send a POST Request

1. Change the method to **POST**.
2. Set the URL to `https://jsonplaceholder.typicode.com/posts`.
3. Click the **Body** tab and select **JSON**.
4. Enter a JSON body:

```json
{
  "title": "Hello from Nexus",
  "body": "Testing the API client",
  "userId": 1
}
```

5. Click **Send**. The response shows the created resource.

## 4. Create a Collection

1. In the left sidebar, make sure the **Collections** tab is selected.
2. Click the **+** button next to "Collections".
3. Type a name (e.g., "JSONPlaceholder") and press Enter.

## 5. Save a Request to a Collection

1. After sending a request, click **Save ▾** in the URL bar.
2. Select the target collection from the dropdown.
3. Enter a name for the request (e.g., "Get Post").
4. Click **Confirm**.

The request now appears in the collection tree. Future edits can be saved with a single click on **Save**.

## 6. Create an Environment

1. Click the **Envs** tab in the sidebar.
2. Click **+ New Environment**.
3. Name it "Development".
4. Add a variable: key = `base_url`, value = `https://jsonplaceholder.typicode.com`.
5. Click **Save**.

## 7. Use Environment Variables

1. Select "Development" from the environment dropdown in the toolbar.
2. In your request URL, replace the base URL with `{{base_url}}`:

```
{{base_url}}/posts/1
```

3. Send the request. Nexus resolves `{{base_url}}` to the value from your active environment.

You can use `{{variable}}` syntax in URLs, headers, query parameters, request bodies, and authentication fields.

## 8. View Request History

1. Click the **History** tab in the sidebar.
2. Every request you've sent is logged with method, URL, status, and timestamp.
3. Click any entry to re-open it as a new request tab.

## Next Steps

- [Request Builder](../features/request-builder.md) — Full guide to composing requests
- [Environments & Variables](../features/environments.md) — Variable scopes and dynamic variables
- [Scripting](../scripting/scripts.md) — Automate with pre-request and post-response scripts
- [Keyboard Shortcuts](../productivity/keyboard-shortcuts.md) — Speed up your workflow
