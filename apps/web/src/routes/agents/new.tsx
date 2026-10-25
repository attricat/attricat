import { createFileRoute } from '@tanstack/react-router';
import { NewConversationPage } from '../../features/agents/NewConversationPage';
import { RequireCapability } from '../../features/auth/RequireCapability';
import { capabilities } from '../../features/auth/capabilities';

const NewConversationRoute = () => (
  <RequireCapability capability={capabilities.agentsRun}>
    <NewConversationPage />
  </RequireCapability>
);

export const Route = createFileRoute('/agents/new')({
  component: NewConversationRoute,
});
