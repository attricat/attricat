import { createFileRoute } from '@tanstack/react-router';
import { RuleInspectionPage } from '../../../features/rules/RuleInspectionPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

export const Route = createFileRoute('/manage/rules/runs')({
  component: () => (
    <RequireCapability capability={capabilities.rulesRead}>
      <RuleInspectionPage section="runs" />
    </RequireCapability>
  ),
});
