import { describe, expect, it } from 'vitest';
import { getVisibleOnboardingSteps } from './onboardingSteps';

const stepRoutes = (
  capabilities: Parameters<typeof getVisibleOnboardingSteps>[0],
) => getVisibleOnboardingSteps(capabilities).map((step) => step.to);

describe('getVisibleOnboardingSteps', () => {
  it('shows every step to workspace administrators', () => {
    expect(
      stepRoutes({
        blueprints_write: true,
        extensions_read: true,
        members_manage: true,
      }),
    ).toEqual([
      '/manage/blueprints/new',
      '/manage/contexts',
      '/manage/workspace/invitations',
      '/manage/extensions',
    ]);
  });

  it('omits administration steps the user cannot perform', () => {
    expect(stepRoutes({ blueprints_write: true })).toEqual([
      '/manage/blueprints/new',
      '/manage/contexts',
    ]);
  });

  it('shows no steps to users who cannot create blueprints', () => {
    expect(
      stepRoutes({
        blueprints_write: false,
        extensions_read: true,
        members_manage: true,
      }),
    ).toEqual([]);
    expect(stepRoutes(undefined)).toEqual([]);
  });
});
