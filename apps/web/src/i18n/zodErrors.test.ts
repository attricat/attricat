import i18n from 'i18next';
import { afterEach, describe, expect, it } from 'vitest';
import { z } from 'zod';
import '.';

const message = (schema: z.ZodType, value: unknown) =>
  schema.safeParse(value).error?.issues[0]?.message;

describe('zod error messages', () => {
  afterEach(() => i18n.changeLanguage('en'));

  it('describes input limits in words', () => {
    expect(message(z.string().min(1), '')).toBe('This field is required.');
    expect(message(z.string().max(3), 'four')).toBe(
      'Use at most 3 characters.',
    );
    expect(message(z.array(z.string()).min(1), [])).toBe(
      'Choose at least 1 item.',
    );
    expect(message(z.number().max(10), 11)).toBe('Enter 10 or less.');
    expect(message(z.email(), 'nope')).toBe('Enter a valid email address.');
  });

  it('translates them and keeps messages schemas set themselves', async () => {
    await i18n.changeLanguage('pl');
    expect(message(z.string().min(5), 'ab')).toBe('Użyj co najmniej 5 znaków.');
    expect(message(z.string().min(1, { error: 'Own message' }), '')).toBe(
      'Own message',
    );
  });

  it('leaves other failures to Zod', () => {
    expect(message(z.string(), 1)).toMatch(/expected string/);
  });
});
