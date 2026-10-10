import { createFileRoute } from '@tanstack/react-router';
import { PasswordLoginPage } from '../../features/auth/LoginPage';

const LoginRoute = () => (
  <PasswordLoginPage identifier={Route.useParams().identifier} />
);

export const Route = createFileRoute('/login/$identifier')({
  component: LoginRoute,
});
