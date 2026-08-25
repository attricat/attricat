import { createFileRoute } from '@tanstack/react-router';
import { WorkspaceLoginPage } from '../features/auth/LoginPage';

export const Route = createFileRoute('/login')({
  component: WorkspaceLoginPage,
});
