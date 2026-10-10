import { describe, expect, it } from 'vitest';
import { documentationUrl } from './documentation';

describe('documentationUrl', () => {
  it('links English readers to the root locale', () => {
    expect(documentationUrl('home', 'en')).toBe('https://docs.attricat.com/');
    expect(documentationUrl('blueprints', 'en')).toBe(
      'https://docs.attricat.com/builders/blueprints/',
    );
  });

  it('links Polish readers to the Polish locale', () => {
    expect(documentationUrl('contexts', 'pl')).toBe(
      'https://docs.attricat.com/pl/guides/contexts/',
    );
  });

  it('falls back to the root locale for unsupported languages', () => {
    expect(documentationUrl('home', 'de')).toBe('https://docs.attricat.com/');
    expect(documentationUrl('home')).toBe('https://docs.attricat.com/');
  });
});
