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
      version={z.coerce.number().int().positive().parse(version)}
    />
  );
}
