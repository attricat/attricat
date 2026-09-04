import { createFileRoute } from '@tanstack/react-router';
import { ExtensionsPage } from '../../../features/extensions/ExtensionsPage';

export const Route = createFileRoute('/manage/extensions/')({
  component: ExtensionsPage,
});
