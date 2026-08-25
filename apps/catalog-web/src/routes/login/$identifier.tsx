import { createFileRoute } from '@tanstack/react-router';
import { PasswordLoginPage } from '../../features/auth/LoginPage';

export const Route = createFileRoute('/login/$identifier')({
  component: () => (
    <PasswordLoginPage identifier={Route.useParams().identifier} />
  ),
});
