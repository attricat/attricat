import { createFileRoute, notFound } from '@tanstack/react-router';
import { UiKitGallery } from '../../../features/extensions/native/UiKitGallery';

// A development tool: every UI kit element as the host draws it.
export const Route = createFileRoute('/manage/extensions/ui-kit')({
  beforeLoad: () => {
    if (!import.meta.env.DEV) throw notFound();
  },
  component: UiKitGallery,
});
