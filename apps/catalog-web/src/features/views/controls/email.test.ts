import { describe, expect, it } from 'vitest';
import { emailHref, isEmailAddress, validateEmail } from './email';
import { validatesJsonSchema } from '../../entities/jsonSchema';

const valid = [
  'Name+tag@Example.com',
  'a@localhost',
  "o'brien@example.test",
  'a?b#c@example.test',
];
const invalid = [
  '',
  'name',
  '.a@example.test',
  'a..b@example.test',
  'a@-example.test',
  'a@example..test',
  ' a@example.test',
  'a@example.test\n',
  'a@example.test\r\nBcc:other@example.test',
  'mailto:a@example.test',
  'Name <a@example.test>',
  'a@example.test,b@example.test',
  'a@example.test?subject=hello',
  '<script>@example.test',
  'ü@example.test',
  'a@例子.test',
  `${'a'.repeat(65)}@example.test`,
  `a@${'a'.repeat(64)}.test`,
];

describe('email values', () => {
  it.each(valid)('accepts a single ASCII address: %s', (value) => {
    expect(isEmailAddress(value)).toBe(true);
    expect(
      validatesJsonSchema(value, { type: 'string', format: 'email' }),
    ).toBe(true);
  });
  it.each(invalid)('rejects unsupported or unsafe input: %s', (value) => {
    expect(isEmailAddress(value)).toBe(false);
    expect(emailHref(value)).toBeUndefined();
    expect(
      validatesJsonSchema(value, { type: 'string', format: 'email' }),
    ).toBe(false);
  });
  it('encodes recipient characters rather than treating them as headers', () => {
    expect(emailHref('a?b#c@example.test')).toBe(
      'mailto:a%3Fb%23c@example.test',
    );
    expect(emailHref('Name+tag@Example.com')).toBe(
      'mailto:Name%2Btag@Example.com',
    );
    expect(emailHref({ email: 'a@example.test' })).toBeUndefined();
  });
  it('validates configured email edits after trimming', () => {
    expect(validateEmail('')).toBeUndefined();
    expect(validateEmail(' Name+tag@Example.com ')).toBeUndefined();
    expect(validateEmail('not an email')).toMatch(/single email address/);
  });
});
