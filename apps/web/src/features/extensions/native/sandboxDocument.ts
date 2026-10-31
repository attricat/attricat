import { nativeMessageTypes as message } from './constants';

const bootstrapNonce = 'attricat-native-bootstrap';

/**
 * The one hidden, opaque-origin document that hosts every extension's native
 * Worker for the session. It never runs extension code itself: each start
 * request becomes a dedicated Worker from the artifact, handed only that
 * extension's MessagePort. Workers are classic scripts: browsers refuse
 * module Workers from `blob:` URLs in an opaque origin. A Worker created from
 * a `blob:` URL inherits this document's opaque origin and policy, so it
 * cannot reach the network, load code or open storage either
 * (`scripts/verify-native-sandbox.mjs` checks this in a browser).
 */
export const nativeSandboxDocument = `<!doctype html><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-${bootstrapNonce}' blob:; worker-src blob:; connect-src 'none'"><script nonce="${bootstrapNonce}">
(() => { const workers = new Map(); let control;
const stop = key => { const worker = workers.get(key); if (!worker) return; worker.terminate(); URL.revokeObjectURL(worker.url); workers.delete(key); };
window.addEventListener('message', event => { if (event.source !== parent || event.data?.type !== '${message.sandboxInit}' || !event.ports[0] || control) return; control = event.ports[0];
control.onmessage = ({ data, ports }) => { if (data?.type === '${message.stop}') { stop(data.key); return; } if (data?.type !== '${message.start}' || typeof data.key !== 'string' || typeof data.artifact !== 'string' || !ports[0]) return; stop(data.key);
const url = URL.createObjectURL(new Blob([data.artifact], { type: 'text/javascript' })); let worker; try { worker = new Worker(url, { name: data.key }); } catch { URL.revokeObjectURL(url); control.postMessage({ type: '${message.workerError}', key: data.key }); return; } worker.url = url; workers.set(data.key, worker);
worker.onerror = () => control.postMessage({ type: '${message.workerError}', key: data.key }); worker.postMessage({ type: '${message.workerInit}' }, [ports[0]]); }; }); })();
</script>`;
