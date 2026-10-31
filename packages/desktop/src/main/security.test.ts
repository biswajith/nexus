import { describe, it, expect } from 'vitest';
import { isAppUrl, isExternalWebUrl, isTrustedIpcSender } from './security.js';

const fileApp = 'file:///opt/nexus/dist/renderer/index.html';
const devApp = 'http://localhost:5173';

describe('isAppUrl', () => {
  it('accepts the bundled page with a hash or query', () => {
    expect(isAppUrl(fileApp, fileApp)).toBe(true);
    expect(isAppUrl(`${fileApp}#/collections`, fileApp)).toBe(true);
    expect(isAppUrl(`${fileApp}?x=1`, fileApp)).toBe(true);
  });

  it('rejects other local files, such as a file dropped onto the window', () => {
    expect(isAppUrl('file:///home/user/Downloads/evil.html', fileApp)).toBe(false);
    expect(isAppUrl('file:///opt/nexus/dist/renderer/other.html', fileApp)).toBe(false);
  });

  it('rejects web pages and other schemes', () => {
    expect(isAppUrl('https://attacker.test/', fileApp)).toBe(false);
    expect(isAppUrl('data:text/html,<script>1</script>', fileApp)).toBe(false);
    expect(isAppUrl('javascript:alert(1)', fileApp)).toBe(false);
    expect(isAppUrl('not a url', fileApp)).toBe(false);
  });

  it('matches the dev server by origin only', () => {
    expect(isAppUrl('http://localhost:5173/src/main.tsx', devApp)).toBe(true);
    expect(isAppUrl('http://localhost:5174/', devApp)).toBe(false);
    expect(isAppUrl('http://127.0.0.1:5173/', devApp)).toBe(false);
  });
});

describe('isTrustedIpcSender', () => {
  it('accepts the app page in the main frame', () => {
    expect(isTrustedIpcSender({ url: fileApp, isMainFrame: true }, fileApp)).toBe(true);
  });

  it('rejects subframes even on the app URL', () => {
    expect(isTrustedIpcSender({ url: fileApp, isMainFrame: false }, fileApp)).toBe(false);
  });

  it('rejects a main frame that navigated away', () => {
    expect(isTrustedIpcSender({ url: 'file:///tmp/evil.svg', isMainFrame: true }, fileApp)).toBe(false);
    expect(isTrustedIpcSender({ url: undefined, isMainFrame: true }, fileApp)).toBe(false);
  });
});

describe('isExternalWebUrl', () => {
  it('allows http and https only', () => {
    expect(isExternalWebUrl('https://example.com')).toBe(true);
    expect(isExternalWebUrl('http://example.com')).toBe(true);
    expect(isExternalWebUrl('file:///etc/passwd')).toBe(false);
    expect(isExternalWebUrl('smb://host/share')).toBe(false);
    expect(isExternalWebUrl('javascript:alert(1)')).toBe(false);
  });
});
