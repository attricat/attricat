import { createFileRoute } from '@tanstack/react-router';
import { ReusableAttributeEditorPage } from '../../../features/reusable-attributes/ReusableAttributeEditorPage';

export const Route = createFileRoute('/manage/reusable-attributes/new')({
  component: ReusableAttributeEditorPage,
});
