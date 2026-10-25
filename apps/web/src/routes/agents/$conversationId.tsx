import { createFileRoute } from '@tanstack/react-router';
import { ConversationDetailPage } from '../../features/agents/ConversationDetailPage';
import { RequireCapability } from '../../features/auth/RequireCapability';
import { capabilities } from '../../features/auth/capabilities';

const ConversationRoute = () => (
  <RequireCapability capability={capabilities.agentsRun}>
    <ConversationDetailPage conversationId={Route.useParams().conversationId} />
  </RequireCapability>
);

export const Route = createFileRoute('/agents/$conversationId')({
  component: ConversationRoute,
});
