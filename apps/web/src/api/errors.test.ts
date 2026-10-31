import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import i18n from 'i18next';
import contract from '../../../../contracts/api-error-codes.json';
import en from '../i18n/locales/en.json';
import { errorMessage } from './errors';
import { ApiRequestError } from './request';
import '../i18n';

describe('API error messages', () => {
  it('shows a schema failure as its issue messages, not as JSON', () => {
    const result = z
      .object({ code: z.string().regex(/^[a-z]+$/, 'Use lowercase letters') })
      .safeParse({ code: 'Bad Code!' });
    expect(errorMessage(result.error as Error)).toBe('Use lowercase letters');
  });

  it('translates every code in the API error contract', () => {
    const translated = new Set(Object.keys(en.apiErrors));
    expect(contract.codes.filter((code) => !translated.has(code))).toEqual([]);
    expect(
      Object.keys(en.apiErrors).filter(
        (code) => !contract.codes.includes(code),
      ),
    ).toEqual([]);
  });

  it('translates by code and interpolates scalar details', async () => {
    await i18n.changeLanguage('pl');
    const error = new ApiRequestError(
      409,
      'record is locked',
      'record_locked',
      {
        attribute: 'status',
        context: 'default',
        status: 'approved',
      },
    );
    expect(errorMessage(error)).toBe(
      'Rekord jest zablokowany przez status atrybutu status.',
    );
    await i18n.changeLanguage('en');
  });

  it('keeps the server message for generic codes', () => {
    const error = new ApiRequestError(
      422,
      "field 'name' must not be empty",
      'invalid_input',
    );
    expect(errorMessage(error)).toBe(
      "The request was rejected: field 'name' must not be empty",
    );
  });

  it('falls back to the message without a known code', () => {
    expect(
      errorMessage(new ApiRequestError(500, 'boom', 'unlisted_code')),
    ).toBe('boom');
    expect(errorMessage(new Error('offline'))).toBe('offline');
    expect(errorMessage(undefined)).toBeUndefined();
  });
});
