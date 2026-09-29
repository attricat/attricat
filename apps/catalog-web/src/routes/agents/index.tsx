import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { ConversationsPage } from '../../features/agents/ConversationsPage';
import { maximumSearchLength } from '../../features/agents/constants';

const ConversationListRoute = () => {
  const search = Route.useSearch();
  return <ConversationsPage key={search.q ?? ''} search={search} />;
};

export const Route = createFileRoute('/agents/')({
  validateSearch: z.object({
    q: z.string().max(maximumSearchLength).optional(),
  }),
  component: ConversationListRoute,
});
