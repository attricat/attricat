import { createFileRoute } from '@tanstack/react-router';
import { PasswordSetupPage } from '../features/workspace/WorkspaceManagementPage';

export const Route = createFileRoute('/onboarding')({
  component: PasswordSetupPage,
});
