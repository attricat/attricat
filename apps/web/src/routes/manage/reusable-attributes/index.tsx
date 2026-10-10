import { createFileRoute } from '@tanstack/react-router';
import { ReusableAttributesPage } from '../../../features/reusable-attributes/ReusableAttributesPage';

export const Route = createFileRoute('/manage/reusable-attributes/')({
  component: ReusableAttributesPage,
});
