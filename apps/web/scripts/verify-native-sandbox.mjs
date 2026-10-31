// Checks, in a real browser, that a Worker started by the native sandbox frame
// is as restricted as the frame: no network, no remote code, an opaque origin.
// Run against a Vite dev server: WEB_URL=http://127.0.0.1:$WEB_PORT node scripts/verify-native-sandbox.mjs
import { chromium } from '@playwright/test';

const webUrl = process.env.WEB_URL;
if (!webUrl) throw new Error('Set WEB_URL to a running web dev server');

// The probe runs as an extension's Worker and reports what it could reach.
const probe = `(async () => {
const port = await new Promise((resolve) =>
  self.addEventListener('message', (event) => resolve(event.ports[0]), { once: true }));
const attempt = async (run) => { try { await run(); return 'allowed'; } catch { return 'blocked'; } };
const results = {
  origin: self.origin,
  fetchRemote: await attempt(() => fetch('https://example.com/')),
  fetchHost: await attempt(() => fetch('${webUrl}/api/health')),
  importRemote: await attempt(() => import('https://cdn.jsdelivr.net/npm/preact@10.26.4/dist/preact.mjs')),
  webSocket: await new Promise((resolve) => {
    try {
      const socket = new WebSocket('wss://example.com/');
      socket.onopen = () => resolve('allowed');
      socket.onerror = () => resolve('blocked');
      setTimeout(() => resolve('blocked'), 3000);
    } catch { resolve('blocked'); }
  }),
  indexedDb: await attempt(() => new Promise((resolve, reject) => {
    const request = indexedDB.open('probe');
    request.onsuccess = resolve;
    request.onerror = () => reject(request.error);
  })),
  parentAccess: typeof parent === 'undefined' && typeof document === 'undefined' ? 'none' : 'present',
};
port.postMessage(results);
})();
`;

const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto(webUrl);
const results = await page.evaluate(async (artifact) => {
  const { nativeSandboxDocument } =
    await import('/src/features/extensions/native/sandboxDocument.ts');
  const { nativeMessageTypes } =
    await import('/src/features/extensions/native/constants.ts');
  const frame = document.createElement('iframe');
  frame.sandbox.add('allow-scripts');
  frame.srcdoc = nativeSandboxDocument;
  frame.hidden = true;
  await new Promise((resolve) => {
    frame.addEventListener('load', resolve, { once: true });
    document.body.append(frame);
  });
  const control = new MessageChannel();
  frame.contentWindow.postMessage(
    { type: nativeMessageTypes.sandboxInit },
    '*',
    [control.port2],
  );
  const extension = new MessageChannel();
  const answer = new Promise((resolve) => {
    extension.port1.onmessage = ({ data }) => resolve(data);
  });
  control.port1.postMessage(
    { type: nativeMessageTypes.start, key: 'probe', artifact },
    [extension.port2],
  );
  return Promise.race([
    answer,
    new Promise((resolve) =>
      setTimeout(() => resolve({ timedOut: true }), 15000),
    ),
  ]);
}, probe);
await browser.close();

console.log(JSON.stringify(results, null, 2));
const expected = {
  origin: 'null',
  fetchRemote: 'blocked',
  fetchHost: 'blocked',
  importRemote: 'blocked',
  webSocket: 'blocked',
  indexedDb: 'blocked',
  parentAccess: 'none',
};
const failures = Object.entries(expected).filter(
  ([key, value]) => results[key] !== value,
);
if (failures.length) {
  console.error('Unexpected:', failures.map(([key]) => key).join(', '));
  process.exit(1);
}
console.log('The native Worker is isolated like its sandbox frame.');
