import { createFileRoute } from '@tanstack/react-router';
import { RulePage } from '../../../../features/rules/RulePage';
import { RequireCapability } from '../../../../features/auth/RequireCapability';
import { capabilities } from '../../../../features/auth/capabilities';
import { RULE_PAGE_DEFINITION } from '../../../../features/rules/constants';

export const Route = createFileRoute('/manage/rules/$ruleId/')({
  component: RuleDefinitionRoute,
});

function RuleDefinitionRoute() {
  const { ruleId } = Route.useParams();
  return (
    <RequireCapability capability={capabilities.rulesRead}>
      <RulePage ruleId={ruleId} section={RULE_PAGE_DEFINITION} />
    </RequireCapability>
  );
}
