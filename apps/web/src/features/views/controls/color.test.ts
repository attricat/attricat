import { describe, expect, it } from 'vitest';
import { parseColor } from './color';

describe('parseColor', () => {
  it.each(['#000000', '#ffffff', '#12aBcD'])('accepts %s', (value) => {
    expect(parseColor(value)).toBe(value.toLowerCase());
  });
  it.each([
    null,
    undefined,
    123456,
    {},
    [],
    '',
    '#fff',
    '#12345678',
    'red',
    'transparent',
    'rgb(0,0,0)',
    'var(--color)',
    'url(https://example.com)',
    '#12345g',
    ' #123456',
    '#123456\n',
    '#123456 ',
  ])('rejects %j', (value) => {
    expect(parseColor(value)).toBeUndefined();
  });
});
