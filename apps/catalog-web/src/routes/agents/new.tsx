import { createFileRoute } from '@tanstack/react-router';
import { NewConversationPage } from '../../features/agents/NewConversationPage';

export const Route = createFileRoute('/agents/new')({
  component: NewConversationPage,
});
