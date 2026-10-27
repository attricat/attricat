import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { RuleEditorPage } from '../../../../../../features/rules/RuleEditorPage';
import { RequireCapability } from '../../../../../../features/auth/RequireCapability';
import { capabilities } from '../../../../../../features/auth/capabilities';

export const Route = createFileRoute(
  '/manage/rules/$ruleId/revisions/$version/new',
)({
  component: RuleRevisionEditorRoute,
});

function RuleRevisionEditorRoute() {
  const { ruleId, version } = Route.useParams();
  return (
    <RequireCapability capability={capabilities.rulesManage}>
      <RuleEditorPage
        ruleId={ruleId}
        sourceVersion={z.coerce.number().int().positive().parse(version)}
      />
    </RequireCapability>
  );
}
