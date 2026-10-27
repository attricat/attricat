import { describe, expect, it } from 'vitest';
import { personName } from './personName';

describe('personName', () => {
  it('falls back when the display name is missing or blank', () => {
    expect(personName('Ada', 'ada@example.com')).toBe('Ada');
    expect(personName(null, 'ada@example.com')).toBe('ada@example.com');
    expect(personName('  ', 'ada@example.com')).toBe('ada@example.com');
  });
});
