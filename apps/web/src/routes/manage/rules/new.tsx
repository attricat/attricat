import { createFileRoute } from '@tanstack/react-router';
import { RuleEditorPage } from '../../../features/rules/RuleEditorPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

const NewRuleRoute = () => (
  <RequireCapability capability={capabilities.rulesManage}>
    <RuleEditorPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/rules/new')({
  component: NewRuleRoute,
});
