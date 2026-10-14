import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { ConversationsPage } from '../../features/agents/ConversationsPage';
import { maximumSearchLength } from '../../features/agents/constants';
import { RequireCapability } from '../../features/auth/RequireCapability';

const ConversationListRoute = () => {
  const search = Route.useSearch();
  return (
    <RequireCapability capability="agents_run">
      <ConversationsPage key={search.q ?? ''} search={search} />
    </RequireCapability>
  );
};

export const Route = createFileRoute('/agents/')({
  validateSearch: z.object({
    q: z.string().max(maximumSearchLength).optional(),
  }),
  component: ConversationListRoute,
});
