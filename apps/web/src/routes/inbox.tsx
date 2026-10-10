import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { InboxPage } from '../features/notifications/InboxPage';
import { inboxFilters } from '../features/notifications/constants';

const InboxRoute = () => {
  const search = Route.useSearch();
  return <InboxPage filter={search.filter ?? 'all'} />;
};

export const Route = createFileRoute('/inbox')({
  validateSearch: z.object({
    filter: z.enum(inboxFilters).optional().catch(undefined),
  }),
  component: InboxRoute,
});
