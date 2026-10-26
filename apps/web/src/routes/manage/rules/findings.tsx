import { createFileRoute } from '@tanstack/react-router';
import { RuleInspectionPage } from '../../../features/rules/RuleInspectionPage';
import {
  findingsSearchSchema,
  type FindingsSearch,
} from '../../../features/rules/schemas';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

const parseFindingsSearch = (input: Record<string, unknown>): FindingsSearch =>
  findingsSearchSchema.parse(input);

const FindingsRoute = () => (
  <RequireCapability capability={capabilities.rulesRead}>
    <RuleInspectionPage findingsSearch={Route.useSearch()} section="findings" />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/rules/findings')({
  validateSearch: parseFindingsSearch,
  component: FindingsRoute,
});
