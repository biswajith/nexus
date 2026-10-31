import * as fs from 'node:fs/promises';

/** Mode for files that may hold credentials: owner read/write only. */
export const PRIVATE_FILE_MODE = 0o600;

/** Mode for directories that hold such files: owner only. */
export const PRIVATE_DIR_MODE = 0o700;

/**
 * Creates `dir` (and parents) if needed and restricts it to the owner.
 * `mkdir`'s mode only applies to newly created directories, so the leaf is also `chmod`ed.
 * @param dir - Directory to create or tighten.
 */
export async function ensurePrivateDir(dir: string): Promise<void> {
  await fs.mkdir(dir, { recursive: true, mode: PRIVATE_DIR_MODE });
  await fs.chmod(dir, PRIVATE_DIR_MODE);
}

/**
 * Writes `data` to `file` with mode 0600. The mode passed to `open` only applies when the file
 * is created, so the handle is also `chmod`ed to fix files left behind by older versions.
 * @param file - Destination path.
 * @param data - File contents.
 * @param flag - `open` flag: `'w'` (default) replaces, `'wx'` fails if the file exists, `'a'` appends.
 */
export async function writePrivateFile(file: string, data: string, flag: 'w' | 'wx' | 'a' = 'w'): Promise<void> {
  const handle = await fs.open(file, flag, PRIVATE_FILE_MODE);
  try {
    await handle.chmod(PRIVATE_FILE_MODE);
    await handle.writeFile(data, 'utf-8');
  } finally {
    await handle.close();
  }
}

/**
 * Appends `data` to `file`, creating it with mode 0600.
 * @param file - Destination path.
 * @param data - Text to append.
 */
export async function appendPrivateFile(file: string, data: string): Promise<void> {
  await writePrivateFile(file, data, 'a');
}

/**
 * Whether `id` can be used as a file name as-is: letters, digits, `_` and `-` only.
 * IDs of imported records come from the imported file, so anything else could contain
 * path separators or `..`.
 * @param id - Candidate identifier.
 */
export function isSafeFileId(id: unknown): id is string {
  return typeof id === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(id);
}
