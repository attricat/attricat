import { createFileRoute } from '@tanstack/react-router';
import { versionParamSchema } from '../../../../../../api/pathSegments';
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
        // A malformed version shows the page's not-found state.
        sourceVersion={versionParamSchema.safeParse(version).data}
      />
    </RequireCapability>
  );
}
