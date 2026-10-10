import { createFileRoute } from '@tanstack/react-router';
import { RuleInspectionPage } from '../../../features/rules/RuleInspectionPage';

export const Route = createFileRoute('/manage/rules/')({
  component: () => <RuleInspectionPage section="rules" />,
});
