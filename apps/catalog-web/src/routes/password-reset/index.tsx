import { createFileRoute } from '@tanstack/react-router';
import { PasswordResetRequestPage } from '../../features/auth/PasswordResetPages';

export const Route = createFileRoute('/password-reset/')({
  component: PasswordResetRequestPage,
});
