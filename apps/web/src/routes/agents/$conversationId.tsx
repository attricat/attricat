import { createFileRoute } from '@tanstack/react-router';
import { ConversationDetailPage } from '../../features/agents/ConversationDetailPage';

const ConversationRoute = () => (
  <ConversationDetailPage conversationId={Route.useParams().conversationId} />
);

export const Route = createFileRoute('/agents/$conversationId')({
  component: ConversationRoute,
});
