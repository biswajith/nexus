/**
 * Window and IPC trust policy for the main process.
 *
 * The renderer's preload exposes the full `window.nexus` bridge (file access, MCP stdio spawn),
 * so only the app's own page may hold it. These checks are pure so they can be unit-tested
 * without Electron; `index.ts` wires them into the window.
 */

/**
 * Whether `url` is the app's own page (same document, any hash or query).
 * @param url - URL being navigated to, or the URL of an IPC sender frame.
 * @param appUrl - URL the main window was loaded with (`file://…/index.html` or the dev server).
 */
export function isAppUrl(url: string, appUrl: string): boolean {
  let target: URL;
  let app: URL;
  try {
    target = new URL(url);
    app = new URL(appUrl);
  } catch {
    return false;
  }
  if (app.protocol === 'file:') {
    return target.protocol === 'file:' && target.pathname === app.pathname;
  }
  return target.origin === app.origin;
}

/**
 * Whether an IPC message comes from the app's own top-level page. Subframes (such as the
 * response preview, visualizer and docs iframes) and any navigated-away page are rejected.
 * @param sender - The sender frame's URL and whether it is the window's main frame.
 * @param appUrl - URL the main window was loaded with.
 */
export function isTrustedIpcSender(
  sender: { url: string | undefined; isMainFrame: boolean },
  appUrl: string,
): boolean {
  return sender.isMainFrame && typeof sender.url === 'string' && isAppUrl(sender.url, appUrl);
}

/**
 * Whether a link the page tries to open in a new window may be handed to the OS browser.
 * Only web URLs qualify; `file:`, custom schemes and `javascript:` are refused.
 * @param url - URL from `window.open` or a `target="_blank"` link.
 */
export function isExternalWebUrl(url: string): boolean {
  try {
    const { protocol } = new URL(url);
    return protocol === 'https:' || protocol === 'http:';
  } catch {
    return false;
  }
}
