import { createFileRoute } from '@tanstack/react-router';
import { RulePage } from '../../../../features/rules/RulePage';
import { RequireCapability } from '../../../../features/auth/RequireCapability';
import { capabilities } from '../../../../features/auth/capabilities';
import { RULE_PAGE_CHANGELOG } from '../../../../features/rules/constants';
import {
  ruleComparisonSearchSchema,
  type RuleComparisonSearch,
} from '../../../../features/rules/schemas';

const parseComparisonSearch = (
  input: Record<string, unknown>,
): RuleComparisonSearch => ruleComparisonSearchSchema.parse(input);

export const Route = createFileRoute('/manage/rules/$ruleId/changelog')({
  validateSearch: parseComparisonSearch,
  component: RuleChangelogRoute,
});

function RuleChangelogRoute() {
  const { ruleId } = Route.useParams();
  return (
    <RequireCapability capability={capabilities.rulesRead}>
      <RulePage
        comparison={Route.useSearch()}
        ruleId={ruleId}
        section={RULE_PAGE_CHANGELOG}
      />
    </RequireCapability>
  );
}
