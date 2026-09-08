import { createFileRoute } from '@tanstack/react-router';
import { InstalledExtensionPage } from '../../../features/extensions/InstalledExtensionPage';

const Page = () => (
  <InstalledExtensionPage extensionId={Route.useParams().extensionId} />
);
export const Route = createFileRoute('/manage/extensions/$extensionId')({
  component: Page,
});
