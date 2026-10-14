import { createFileRoute } from '@tanstack/react-router';
import { ConversationDetailPage } from '../../features/agents/ConversationDetailPage';
import { RequireCapability } from '../../features/auth/RequireCapability';

const ConversationRoute = () => (
  <RequireCapability capability="agents_run">
    <ConversationDetailPage conversationId={Route.useParams().conversationId} />
  </RequireCapability>
);

export const Route = createFileRoute('/agents/$conversationId')({
  component: ConversationRoute,
});
