import { createFileRoute } from '@tanstack/react-router';
import { RuleEditorPage } from '../../../../features/rules/RuleEditorPage';
import { RequireCapability } from '../../../../features/auth/RequireCapability';
import { capabilities } from '../../../../features/auth/capabilities';

export const Route = createFileRoute('/manage/rules/$ruleId/edit')({
  component: RuleEditRoute,
});

function RuleEditRoute() {
  const { ruleId } = Route.useParams();
  return (
    <RequireCapability capability={capabilities.rulesManage}>
      <RuleEditorPage ruleId={ruleId} />
    </RequireCapability>
  );
}
