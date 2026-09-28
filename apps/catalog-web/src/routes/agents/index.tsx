import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { ConversationsPage } from '../../features/agents/ConversationsPage';

const ConversationListRoute = () => {
  const search = Route.useSearch();
  return <ConversationsPage key={search.q ?? ''} search={search} />;
};

export const Route = createFileRoute('/agents/')({
  validateSearch: z.object({ q: z.string().max(120).optional() }),
  component: ConversationListRoute,
});
