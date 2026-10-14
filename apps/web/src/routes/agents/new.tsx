import { createFileRoute } from '@tanstack/react-router';
import { NewConversationPage } from '../../features/agents/NewConversationPage';
import { RequireCapability } from '../../features/auth/RequireCapability';

const NewConversationRoute = () => (
  <RequireCapability capability="agents_run">
    <NewConversationPage />
  </RequireCapability>
);

export const Route = createFileRoute('/agents/new')({
  component: NewConversationRoute,
});
