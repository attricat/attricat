import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { PasswordResetConfirmationPage } from '../../features/auth/PasswordResetPages';

export const Route = createFileRoute('/password-reset/confirm')({
  validateSearch: z.object({ token: z.string().optional() }),
  component: PasswordResetConfirmationRoute,
});

function PasswordResetConfirmationRoute() {
  return <PasswordResetConfirmationPage token={Route.useSearch().token} />;
}
