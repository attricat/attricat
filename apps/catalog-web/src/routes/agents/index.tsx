import { createFileRoute } from '@tanstack/react-router';
import { ConversationsPage } from '../../features/agents/ConversationsPage';

export const Route = createFileRoute('/agents/')({
  component: ConversationsPage,
});
