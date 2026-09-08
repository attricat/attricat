import { describe, expect, it } from 'vitest';
import { frameDocument } from './ExtensionFrame';

describe('extension frame bootstrap', () => {
  it('permits only its nonce-authorized bootstrap and blob extension module', () => {
    expect(frameDocument).toContain(
      "script-src 'nonce-catalog-bootstrap' blob:",
    );
    expect(frameDocument).toContain('<script nonce="catalog-bootstrap">');
    expect(frameDocument).not.toContain("script-src 'unsafe-inline'");
    expect(frameDocument).toContain("connect-src 'none'");
  });

  it('uses a versioned MessageChannel protocol rather than direct network access', () => {
    expect(frameDocument).toContain("type:'catalog:request.v1'");
    expect(frameDocument).toContain("message?.type !== 'catalog:response.v1'");
    expect(frameDocument).toContain("connect-src 'none'");
  });

  it('mounts the artifact inside the frame and delivers context through its port', () => {
    expect(frameDocument).toContain('catalog:context-update.v1');
    expect(frameDocument).toContain('catalog:context-changed.v1');
    expect(frameDocument).toContain('catalog:shutdown.v1');
    expect(frameDocument).toContain('module.mount(root, globalThis.catalog)');
    expect(frameDocument).not.toContain(
      'document.createElement(event.data.element)',
    );
    expect(frameDocument).not.toContain("window.addEventListener('popstate'");
  });

  it('exposes storage only through the MessageChannel broker', () => {
    expect(frameDocument).toContain(
      "storage: { get: detail => call('storage.get'",
    );
    expect(frameDocument).toContain("call('storage.set'");
    expect(frameDocument).toContain("call('storage.delete'");
    expect(frameDocument).toContain("call('storage.list'");
  });
});
