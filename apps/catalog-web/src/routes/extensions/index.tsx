import { createFileRoute } from '@tanstack/react-router';
import { AppsPage } from '../../features/extensions/AppsPage';

export const Route = createFileRoute('/extensions/')({
  component: AppsPage,
});
