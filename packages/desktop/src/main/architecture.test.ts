/**
 * Architecture rules (ArchUnitTS) for the Electron main process, guarding the security-audit
 * findings that live here. The rules scan the sources in `tsconfig.main.json`, which excludes
 * `*.test.ts`. Run with `npm test --workspace=packages/desktop`.
 */
import { describe, it, expect } from 'vitest';
import { projectFiles } from 'archunit';
import type { FileInfo } from 'archunit';

const TSCONFIG = 'tsconfig.main.json';

/** Strips comments so rules match code, not the prose that explains it. */
function code(file: FileInfo): string {
  return file.content
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:'"`])\/\/.*$/gm, '$1');
}

const mainFiles = () => projectFiles(TSCONFIG).inFolder('src/main');
const fileNamed = (name: string) => projectFiles(TSCONFIG).inFolder('src/main').withName(name);
// withName('index.ts') would also match src/preload/index.ts, which the main config pulls in.
const mainEntry = () => projectFiles(TSCONFIG).inPath('src/main/index.ts');

describe('Finding: main window has no navigation guard (needs validation)', () => {
  it('only main/index.ts creates windows, so every window is hardened', async () => {
    const rule = mainFiles().should().adhereTo(
      (f) => f.path.endsWith('main/index.ts') || !/new BrowserWindow\(/.test(code(f)),
      'create BrowserWindows in main/index.ts and pass each one to hardenWindow()',
    );
    await expect(rule).toPassAsync();
  });

  it('every window is hardened before it loads content', async () => {
    const rule = mainEntry().should().adhereTo(
      (f) => {
        const src = code(f);
        const created = (src.match(/new BrowserWindow\(/g) ?? []).length;
        const hardened = (src.match(/hardenWindow\(mainWindow\)/g) ?? []).length;
        return created > 0 && hardened === created && src.indexOf('hardenWindow(mainWindow)') < src.indexOf('.loadURL(');
      },
      'call hardenWindow() on each new BrowserWindow before loadURL/loadFile',
    );
    await expect(rule).toPassAsync();
  });

  it('the hardening blocks navigation, redirects, new windows, webviews and permissions', async () => {
    const rule = mainEntry().should().adhereTo(
      (f) => ["'will-navigate'", "'will-redirect'", "'will-attach-webview'", 'setWindowOpenHandler(', 'setPermissionRequestHandler(']
        .every((needle) => code(f).includes(needle)),
      'a dropped or linked file must never replace the app page that holds window.nexus',
    );
    await expect(rule).toPassAsync();
  });

  it('IPC handlers are registered only through the sender-checking guard', async () => {
    const rule = mainFiles().should().adhereTo(
      (f) => {
        const src = code(f);
        const importsIpcMainValue = /import\s*\{[^}]*\bipcMain\b[^}]*\}\s*from\s*'electron'/.test(src)
          && !/import\s+type\s*\{[^}]*\bipcMain\b/.test(src);
        if (f.path.endsWith('main/index.ts')) {
          return /registerIpcHandlers\(guardIpc\(ipcMain\)\)/.test(src) && !/registerIpcHandlers\(ipcMain\)/.test(src);
        }
        return !importsIpcMainValue;
      },
      'handlers must receive the guarded IpcMain; only index.ts may touch the raw ipcMain',
    );
    await expect(rule).toPassAsync();
  });

  it('renderer isolation settings are never weakened', async () => {
    const rule = mainFiles().should().adhereTo(
      (f) => !/nodeIntegration:\s*true|contextIsolation:\s*false|sandbox:\s*false|webSecurity:\s*false|allowRunningInsecureContent:\s*true/
        .test(code(f)),
      'keep contextIsolation, sandbox and webSecurity on and nodeIntegration off',
    );
    await expect(rule).toPassAsync();
  });

  it('the trust policy stays free of Electron so it can be unit-tested', async () => {
    const rule = fileNamed('security.ts').should().adhereTo(
      (f) => !/from\s*'electron'/.test(code(f)),
      'security.ts holds pure URL checks; wire them up in index.ts',
    );
    await expect(rule).toPassAsync();
  });
});

describe('Finding: credential files created world-readable (needs validation)', () => {
  it('the main process writes files only through the 0600 helpers', async () => {
    const rule = mainFiles().should().adhereTo(
      (f) => !/fs\.(writeFile|appendFile|mkdir|open)\(|writeFileSync|appendFileSync|createWriteStream/.test(code(f)),
      'use writePrivateFile/ensurePrivateDir from @nexus/core',
    );
    await expect(rule).toPassAsync();
  });
});

describe('Finding: import overwrites existing records (medium)', () => {
  it('record ids are validated before they become file names', async () => {
    const rule = mainFiles().should().adhereTo(
      (f) => !/\.id\}\.json|Id\}\.json/.test(code(f)) || code(f).includes('isSafeFileId('),
      'ids from imported files can contain ../; check isSafeFileId() before using one in a path',
    );
    await expect(rule).toPassAsync();
  });

  it('imported environments never replace existing ones', async () => {
    const rule = fileNamed('ipc-handlers.ts').should().adhereTo(
      (f) => {
        const src = code(f);
        const start = src.indexOf("'import:save'");
        const handler = src.slice(start, src.indexOf('ipcMain.handle(', start + 1));
        return start >= 0 && handler.includes('saveImportedEnvironment(') && !handler.includes("'w')");
      },
      "import:save must use saveImportedEnvironment (create-only, fresh id on clash)",
    );
    await expect(rule).toPassAsync();
  });
});
