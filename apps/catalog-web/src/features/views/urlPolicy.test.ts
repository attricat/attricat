import { describe, expect, it } from 'vitest';
import { safeUrl } from './urlPolicy';
import { urlEditComponent } from './components/urlComponents';

describe('URL navigation policy', () => {
  it.each([
    'https://example.com',
    'HTTP://example.com:8080/a?q=hello%20world#part',
    'https://żółw.pl/path',
    'http://localhost',
    'https://[::1]/',
  ])('accepts %s', (value) => {
    expect(safeUrl(value)).toBe(new URL(value).href);
  });
  it.each([
    undefined,
    null,
    123,
    {},
    '',
    'example.com',
    '/path',
    '//example.com',
    'https:///example.com',
    'javascript:alert(1)',
    'data:text/html,hi',
    'mailto:a@example.com',
    'ftp://example.com',
    'https://user:password@example.com',
    'https://user@example.com',
    ' https://example.com',
    'https://example.com\n',
    'https://exa\tmple.com',
    'https://example.com/a b',
    'https://example.com\\evil',
    'https://',
  ])('rejects %j', (value) => {
    expect(safeUrl(value)).toBeUndefined();
  });
  it('validates configured URL edits while allowing optional clearing', () => {
    expect(urlEditComponent.validateValue('')).toBeUndefined();
    expect(
      urlEditComponent.validateValue('https://example.com/a?q=1'),
    ).toBeUndefined();
    expect(urlEditComponent.validateValue('javascript:alert(1)')).toBeTruthy();
  });
});
