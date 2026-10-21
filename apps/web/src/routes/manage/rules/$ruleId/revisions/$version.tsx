import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { RuleRevisionPage } from '../../../../../features/rules/RuleRevisionPage';

export const Route = createFileRoute(
  '/manage/rules/$ruleId/revisions/$version',
)({
  component: RuleRevisionRoute,
});

function RuleRevisionRoute() {
  const { ruleId, version } = Route.useParams();
  return (
    <RuleRevisionPage
      ruleId={ruleId}
      // A malformed version shows the page's not-found state.
      version={z.coerce.number().int().positive().safeParse(version).data}
    />
  );
}
