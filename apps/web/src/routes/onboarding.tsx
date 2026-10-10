import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { PasswordSetupPage } from '../features/workspace/PasswordSetupPage';

const OnboardingRoute = () => {
  const search = Route.useSearch();
  return (
    <PasswordSetupPage
      initialInvitationSecret={search.invitation_secret}
      initialOnboardingSecret={search.onboarding_secret}
    />
  );
};

export const Route = createFileRoute('/onboarding')({
  validateSearch: z.object({
    invitation_secret: z.string().optional(),
    onboarding_secret: z.string().optional(),
  }),
  component: OnboardingRoute,
});
