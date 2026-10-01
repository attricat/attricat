import { describe, expect, it } from 'vitest';
import { phoneHref } from './phoneNumber';

describe('phoneHref', () => {
  it.each([
    ['+48 22 123 45 67', 'tel:+48221234567'],
    [' +1 (202) 555-0123 ext. 0042 ', 'tel:+12025550123;ext=0042'],
    ['+44.20.7946.0958 x123', 'tel:+442079460958;ext=123'],
  ])('builds a dial target without changing stored text: %s', (value, href) => {
    expect(phoneHref(value)).toBe(href);
  });
  it.each([
    '',
    '020 7946 0958',
    '0044 20 7946 0958',
    '+0123456789',
    '+1',
    '+1234567890123456',
    '+123;ext=4',
    '+123#4',
    '+123*4',
    'tel:+123',
    'javascript:alert(1)',
    '+123\n',
    '+123\u0000',
    '+123 ext. abc',
    '+123 x12345678901',
    '+123?',
    '+12' + ' '.repeat(128),
    null,
    undefined,
    123,
    {},
    ['+123'],
  ])('does not turn unsafe or ambiguous data into a link: %s', (value) => {
    expect(phoneHref(value)).toBeUndefined();
  });
});
