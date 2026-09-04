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
});
