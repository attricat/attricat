import { describe, expect, it } from 'vitest';
import { frameDocument } from './frameDocument';

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

  it('exposes refresh only through the MessageChannel broker', () => {
    expect(frameDocument).toContain(
      "refresh: detail => call('refresh', detail)",
    );
  });

  it('applies and announces the color mode for every contribution', () => {
    expect(frameDocument).toContain('; applyTheme(event.data.theme); try {');
    expect(frameDocument).toContain("colorModes = ['light', 'dark']");
    expect(frameDocument).toContain(
      'document.documentElement.style.colorScheme = theme.color_mode',
    );
    expect(frameDocument).toContain(
      "message?.type === 'catalog:theme-update.v1'",
    );
    expect(frameDocument).toContain('catalog:theme-changed.v1');
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
