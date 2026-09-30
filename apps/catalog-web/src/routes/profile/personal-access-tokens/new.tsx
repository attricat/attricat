import { createFileRoute } from '@tanstack/react-router';
import { PersonalTokensPage } from '../../features/profile/PersonalTokensPage';

export const Route = createFileRoute('/profile/personal-access-tokens')({
  component: PersonalTokensPage,
});
