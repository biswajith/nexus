import { app, BrowserWindow, ipcMain, session, shell } from 'electron';
import type { IpcMain, IpcMainInvokeEvent } from 'electron';
import * as path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { registerIpcHandlers } from './ipc-handlers.js';
import { isAppUrl, isExternalWebUrl, isTrustedIpcSender } from './security.js';

app.disableHardwareAcceleration();

const __dirname = path.dirname(fileURLToPath(import.meta.url));

let mainWindow: BrowserWindow | null = null;

const isDev = process.env.NODE_ENV === 'development' || process.argv.includes('--dev');
const rendererFile = path.join(__dirname, '..', 'renderer', 'index.html');
const appUrl = isDev ? 'http://localhost:5173' : pathToFileURL(rendererFile).href;

/**
 * Wraps `ipcMain` so every `handle` callback first checks that the sender is the app's own
 * top-level page. A page reached by navigation, or a subframe, never gets main-process access.
 * @param ipc - Electron's `ipcMain`.
 * @returns An `IpcMain` whose `handle` rejects calls from untrusted frames.
 */
function guardIpc(ipc: IpcMain): IpcMain {
  return Object.create(ipc, {
    handle: {
      value: (channel: string, listener: (event: IpcMainInvokeEvent, ...args: any[]) => unknown) => {
        ipc.handle(channel, (event, ...args) => {
          const frame = event.senderFrame;
          if (!isTrustedIpcSender({ url: frame?.url, isMainFrame: frame === event.sender.mainFrame }, appUrl)) {
            throw new Error(`Blocked ${channel} from untrusted frame`);
          }
          return listener(event, ...args);
        });
      },
    },
  }) as IpcMain;
}

/**
 * Keeps the window on the app page: blocks navigation and redirects elsewhere (for example, a
 * file dropped onto the window), refuses new windows and webviews, and denies permission requests.
 * Web links opened with `target="_blank"` go to the OS browser instead.
 * @param win - The window to harden.
 */
function hardenWindow(win: BrowserWindow): void {
  const contents = win.webContents;
  const blockForeign = (event: { preventDefault: () => void }, url: string) => {
    if (!isAppUrl(url, appUrl)) event.preventDefault();
  };
  contents.on('will-navigate', blockForeign);
  contents.on('will-redirect', blockForeign);
  contents.on('will-attach-webview', (event) => event.preventDefault());
  contents.setWindowOpenHandler(({ url }) => {
    if (isExternalWebUrl(url)) void shell.openExternal(url);
    return { action: 'deny' };
  });
}

/**
 * Creates the main `BrowserWindow`, loads the app URL or bundled HTML, and clears `mainWindow` when the window closes.
 * @returns void
 */
function createWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 960,
    minHeight: 600,
    title: 'Nexus',
    titleBarStyle: 'hiddenInset',
    trafficLightPosition: { x: 16, y: 16 },
    backgroundColor: '#1a1a1a',
    webPreferences: {
      preload: path.join(__dirname, '..', 'preload', 'index.cjs'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  hardenWindow(mainWindow);

  if (isDev) {
    mainWindow.loadURL(appUrl);
    mainWindow.webContents.openDevTools({ mode: 'bottom' });
  } else {
    mainWindow.loadFile(rendererFile);
  }

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

app.whenReady().then(() => {
  // Copy-to-clipboard (cURL, code snippets) is the only browser permission the app uses.
  session.defaultSession.setPermissionRequestHandler((_wc, permission, callback) => {
    callback(permission === 'clipboard-sanitized-write');
  });
  registerIpcHandlers(guardIpc(ipcMain));
  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
