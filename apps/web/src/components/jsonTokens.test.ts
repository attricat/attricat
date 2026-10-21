import { describe, expect, it } from 'vitest';
import { jsonEllipsis, jsonInlineTokens } from './jsonTokens';

const text = (value: unknown, maxLength?: number) =>
  jsonInlineTokens(value, maxLength)
    .map((token) => token.text)
    .join('');

describe('jsonInlineTokens', () => {
  it('spaces one-line JSON and sets keys apart from values', () => {
    const value = { name: 'Widget', size: [1, 2], valid: true, note: null };
    expect(text(value)).toBe(
      '{ "name": "Widget", "size": [1, 2], "valid": true, "note": null }',
    );
    expect(JSON.parse(text(value))).toEqual(value);
    expect(jsonInlineTokens({ a: 'b' })).toEqual([
      { kind: 'punctuation', text: '{ ' },
      { kind: 'key', text: '"a"' },
      { kind: 'punctuation', text: ': ' },
      { kind: 'string', text: '"b"' },
      { kind: 'punctuation', text: ' }' },
    ]);
  });

  it('renders empty containers and scalars as JSON', () => {
    expect(text({})).toBe('{}');
    expect(text([])).toBe('[]');
    expect(text('a "quote"')).toBe('"a \\"quote\\""');
    expect(text(4.5)).toBe('4.5');
  });

  it('stops near the limit with a single ellipsis', () => {
    const tokens = jsonInlineTokens(
      { items: Array.from({ length: 100 }, (_, index) => index) },
      20,
    );
    expect(tokens.at(-1)).toBe(jsonEllipsis);
    expect(tokens.filter((token) => token === jsonEllipsis)).toHaveLength(1);
    expect(text({ long: 'x'.repeat(100) }, 20)).toBe(
      `{ "long": ${'"'.concat('x'.repeat(9))}…`,
    );
  });
});
