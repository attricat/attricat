import { createFileRoute } from '@tanstack/react-router';
import { SideloadExtensionPage } from '../../../features/extensions/SideloadExtensionPage';

export const Route = createFileRoute('/manage/extensions/sideload')({
  component: SideloadExtensionPage,
});
