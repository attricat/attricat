import { createFileRoute } from '@tanstack/react-router';
import { CreatePersonalTokenPage } from '../../../features/profile/CreatePersonalTokenPage';

export const Route = createFileRoute('/profile/personal-access-tokens/new')({
  component: CreatePersonalTokenPage,
});
