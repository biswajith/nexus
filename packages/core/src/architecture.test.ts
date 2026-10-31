/**
 * Architecture rules (ArchUnitTS) that keep the security-audit findings from coming back.
 *
 * Each `describe` block names the finding it guards. The rules scan the production sources
 * listed in `tsconfig.json`, which excludes `*.test.ts`, so test fixtures never trip them.
 * Run with the rest of the suite: `npm test --workspace=packages/core`.
 */
import { describe, it, expect } from 'vitest';
import { projectFiles } from 'archunit';
import type { FileInfo } from 'archunit';

const TSCONFIG = 'tsconfig.json';

/** Strips comments so rules match code, not the prose that explains it. */
function code(file: FileInfo): string {
  return file.content
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:'"`])\/\/.*$/gm, '$1');
}

/** Removes the body of shellQuote(), the one place allowed to build a quoted word by hand. */
function withoutShellQuote(src: string): string {
  return src.replace(/export function shellQuote[\s\S]*?\n\}/, '');
}

const sourceFiles = () => projectFiles(TSCONFIG).withName('*.ts');
const fileNamed = (name: string) => projectFiles(TSCONFIG).withName(name);

describe('Finding: node:vm sandbox escape (critical)', () => {
  it('only scripts/sandbox.ts may use node:vm', async () => {
    const rule = sourceFiles().should().adhereTo(
      (f) => f.path.endsWith('scripts/sandbox.ts') || !/['"](node:)?vm['"]/.test(code(f)),
      'node:vm is not a security boundary; run untrusted code only through ScriptSandbox',
    );
    await expect(rule).toPassAsync();
  });

  it('the in-realm runtime has no imports, so its toString() is self-contained', async () => {
    const rule = fileNamed('sandbox-runtime.ts').should().adhereTo(
      (f) => !/^\s*import\s/m.test(code(f)) && !/\brequire\(/.test(code(f)),
      'sandbox-runtime.ts is injected as source text and must not reference module scope',
    );
    await expect(rule).toPassAsync();
  });

  it('the in-realm runtime does not depend on any other project file', async () => {
    // The graph records a self-edge for every file, so the file itself is excluded from the target.
    const rule = fileNamed('sandbox-runtime.ts')
      .shouldNot()
      .dependOnFiles()
      .inPath(/^src\/(?!scripts\/sandbox-runtime\.ts$)/);
    await expect(rule).toPassAsync();
  });

  it('the sandbox context is created empty and drains microtasks under the timeout', async () => {
    const rule = fileNamed('sandbox.ts').should().adhereTo(
      (f) => {
        const src = code(f);
        return /vm\.createContext\(Object\.create\(null\)/.test(src)
          && /microtaskMode:\s*'afterEvaluate'/.test(src)
          && /sandboxRuntime\.toString\(\)/.test(src);
      },
      'build nx/console inside the context from an empty global; never pass host objects to createContext',
    );
    await expect(rule).toPassAsync();
  });

  it('the sandbox never uses the vm APIs that share or leak the host realm', async () => {
    const rule = sourceFiles().should().adhereTo(
      (f) => !/\b(runInThisContext|runInNewContext)\b/.test(code(f)),
      'runInThisContext runs in the host realm; runInNewContext contextifies a host object',
    );
    await expect(rule).toPassAsync();
  });
});

describe('Finding: cURL codegen shell injection (medium)', () => {
  it('codegen never interpolates values inside hand-written single quotes', async () => {
    const rule = projectFiles(TSCONFIG).inFolder('src/codegen').should().adhereTo(
      (f) => !/'\$\{/.test(withoutShellQuote(code(f))),
      "quote every request-derived value with shellQuote() instead of '${...}'",
    );
    await expect(rule).toPassAsync();
  });

  it('codegen uses curl flags that never read local files from a value', async () => {
    const rule = projectFiles(TSCONFIG).inFolder('src/codegen').should().adhereTo(
      (f) => {
        const src = code(f);
        // -F is allowed only for real file uploads, written as `name=@path`.
        const formFlags = src.match(/[`'"]-F [^\n]*/g) ?? [];
        return !/[`'"]-d /.test(src) && formFlags.every((flag) => flag.includes('=@'));
      },
      'use --data-raw and --form-string: -d @file and -F name=@file read local files',
    );
    await expect(rule).toPassAsync();
  });
});

describe('Finding: import overwrites a collection with the same slug (medium)', () => {
  it('collection creates claim a fresh directory instead of reusing one', async () => {
    const rule = fileNamed('collection-manager.ts').should().adhereTo(
      (f) => {
        const src = code(f);
        const create = src.slice(src.indexOf('async createCollection'), src.indexOf('private async claimDirectory'));
        return create.includes('this.claimDirectory(') && !/mkdir\(collectionDir/.test(create);
      },
      'createCollection must go through claimDirectory so it never writes into an existing collection',
    );
    await expect(rule).toPassAsync();
  });

  it('record ids are validated before they become file names', async () => {
    const rule = sourceFiles().should().adhereTo(
      (f) => !/\.id\}\.json/.test(code(f)) || code(f).includes('isSafeFileId('),
      'ids from imported files can contain ../; check isSafeFileId() before using one in a path',
    );
    await expect(rule).toPassAsync();
  });
});

describe('Finding: credentials forwarded on cross-origin redirects (low)', () => {
  it("nothing uses undici's redirect interceptor or swaps the global dispatcher", async () => {
    const rule = sourceFiles().should().adhereTo(
      (f) => !/interceptors\.redirect|setGlobalDispatcher|maxRedirections/.test(code(f)),
      'undici only strips Authorization/Cookie on redirect; follow redirects in sendHttpRequest instead',
    );
    await expect(rule).toPassAsync();
  });

  it('the request sender strips headers when a redirect changes origin', async () => {
    const rule = fileNamed('request-sender.ts').should().adhereTo(
      (f) => /next\.origin !== new URL\(url\)\.origin/.test(code(f)) && code(f).includes('stripCrossOriginHeaders('),
      'every redirect hop must compare origins and drop non-allowlisted headers',
    );
    await expect(rule).toPassAsync();
  });
});

describe('Finding: credential files created world-readable (needs validation)', () => {
  it('only fs/private-fs.ts writes files', async () => {
    const rule = sourceFiles().should().adhereTo(
      (f) => f.path.endsWith('fs/private-fs.ts')
        || !/\b(writeFile|appendFile|writeFileSync|appendFileSync|createWriteStream)\(|fs\.open\(/.test(
          code(f).replace(/\b(writePrivateFile|appendPrivateFile)\(/g, ''),
        ),
      'use writePrivateFile/appendPrivateFile so tokens, secrets and history are 0600',
    );
    await expect(rule).toPassAsync();
  });

  it('directories are created owner-only', async () => {
    const rule = sourceFiles().should().adhereTo(
      (f) => (code(f).match(/fs\.mkdir\([^;]*\);/g) ?? []).every((call) => /PRIVATE_DIR_MODE/.test(call)),
      'pass mode: PRIVATE_DIR_MODE (or use ensurePrivateDir) to every mkdir',
    );
    await expect(rule).toPassAsync();
  });
});
