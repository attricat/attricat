import { createFileRoute } from '@tanstack/react-router';
import { ReusableAttributeEditorPage } from '../../../features/reusable-attributes/ReusableAttributeEditorPage';

const ReusableAttributeEditorRoute = () => (
  <ReusableAttributeEditorPage definitionId={Route.useParams().definitionId} />
);

export const Route = createFileRoute('/manage/reusable-attributes/$definitionId')({
  component: ReusableAttributeEditorRoute,
});
