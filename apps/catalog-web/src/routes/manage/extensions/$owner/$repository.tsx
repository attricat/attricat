import { createFileRoute } from '@tanstack/react-router';
import { MarketplaceExtensionPage } from '../../../../features/extensions/ExtensionsPage';

const Page = () => {
  const { owner, repository } = Route.useParams();
  return <MarketplaceExtensionPage owner={owner} repository={repository} />;
};
export const Route = createFileRoute('/manage/extensions/$owner/$repository')({
  component: Page,
});
