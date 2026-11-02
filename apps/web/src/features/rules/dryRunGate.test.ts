import { describe, expect, it } from 'vitest';
import { ApiRequestError } from '../../api/request';
import i18n from '../../i18n';
import { canAcceptExistingViolations, dryRunGateMessage } from './dryRunGate';
import {
  RULE_DRY_RUN_REQUIRED,
  RULE_HAS_EXISTING_VIOLATIONS,
} from './constants';

const conflict = (code: string, details?: unknown) =>
  new ApiRequestError(409, 'refused', code, details);

describe('rule dry run gate', () => {
  it('offers to accept found violations and a dry run that hit the limit', () => {
    expect(
      canAcceptExistingViolations(
        conflict(RULE_HAS_EXISTING_VIOLATIONS, { existing_violations: 2 }),
      ),
    ).toBe(true);
    expect(
      canAcceptExistingViolations(
        conflict(RULE_DRY_RUN_REQUIRED, {
          truncated: true,
          existing_violations: 0,
        }),
      ),
    ).toBe(true);
  });

  it('asks for a dry run when none has run', () => {
    expect(canAcceptExistingViolations(conflict(RULE_DRY_RUN_REQUIRED))).toBe(
      false,
    );
  });

  it('explains the limit instead of asking for another dry run', () => {
    expect(
      dryRunGateMessage(
        i18n.t,
        conflict(RULE_DRY_RUN_REQUIRED, {
          truncated: true,
          existing_violations: 7,
        }),
      ),
    ).toContain('record limit');
  });
});
