import { createFileRoute } from '@tanstack/react-router';
import { RuleInspectionPage } from '../../../features/rules/RuleInspectionPage';
import {
  runsSearchSchema,
  type RunsSearch,
} from '../../../features/rules/schemas';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

const parseRunsSearch = (input: Record<string, unknown>): RunsSearch =>
  runsSearchSchema.parse(input);

const RunsRoute = () => (
  <RequireCapability capability={capabilities.rulesRead}>
    <RuleInspectionPage runsSearch={Route.useSearch()} section="runs" />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/rules/runs')({
  validateSearch: parseRunsSearch,
  component: RunsRoute,
});
