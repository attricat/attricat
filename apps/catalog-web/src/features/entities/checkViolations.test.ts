import { describe, expect, it } from 'vitest';
import { ApiRequestError } from '../../api/request';
import {
  checkViolationError,
  publicationReadinessText,
  violationFieldErrors,
} from './checkViolations';
import type { CheckViolation } from './schemas';

const violation = (
  code: string,
  attributes: string[],
  message = `${code} failed`,
): CheckViolation => ({
  source: 'entity_check',
  code,
  message,
  contexts: ['default'],
  attributes,
});

describe('check violations', () => {
  it('reads violations from the declarative check error codes', () => {
    const violations = [violation('valid-range', ['valid_until'])];
    expect(
      checkViolationError(
        new ApiRequestError(
          422,
          'Entity checks failed',
          'entity_check_failed',
          {
            violations,
          },
        ),
      ),
    ).toEqual({ message: 'Entity checks failed', violations });
    expect(
      checkViolationError(
        new ApiRequestError(
          422,
          'Not publishable',
          'publication_checks_failed',
          { violations, context: 'storefront' },
        ),
      ),
    ).toMatchObject({ context: 'storefront' });
  });

  it('ignores other errors and malformed details', () => {
    expect(
      checkViolationError(
        new ApiRequestError(422, 'Bad', 'entity_schema_mismatch', {
          violations: [],
        }),
      ),
    ).toBeUndefined();
    expect(
      checkViolationError(
        new ApiRequestError(422, 'Bad', 'rule_violation', { violations: 1 }),
      ),
    ).toBeUndefined();
    expect(checkViolationError(new Error('Bad'))).toBeUndefined();
  });

  it('places messages on shown fields and summarizes the rest', () => {
    const range = violation('valid-range', ['valid_from', 'valid_until']);
    const sku = violation('sku', ['sku'], 'SKU is required');
    const linked = violation('facility-of-supplier', []);
    const hidden = violation('hidden', ['internal_code']);

    expect(
      violationFieldErrors(
        [
          range,
          sku,
          violation('sku-format', ['sku'], 'Use capitals'),
          linked,
          hidden,
        ],
        ['valid_from', 'valid_until', 'sku'],
      ),
    ).toEqual({
      fieldErrors: {
        valid_from: 'valid-range failed',
        valid_until: 'valid-range failed',
        sku: 'SKU is required Use capitals',
      },
      unplaced: [linked, hidden],
    });
  });

  it('joins the failing channel checks', () => {
    expect(
      publicationReadinessText({
        context_id: '123e4567-e89b-12d3-a456-426614174000',
        context_code: 'storefront',
        ready: false,
        violations: [
          violation('has-sku', [], 'SKU is required.'),
          violation('has-image', [], 'An image is required.'),
        ],
      }),
    ).toBe('SKU is required. An image is required.');
  });
});
