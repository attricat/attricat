import { createFileRoute } from '@tanstack/react-router';
import { ExtensionManagementPage } from '../../../features/extensions/ExtensionManagementPage';
import { ExtensionsMarketplacePage } from '../../../features/extensions/ExtensionsMarketplacePage';

export const Route = createFileRoute('/manage/extensions/marketplace')({
  component: () => (
    <ExtensionManagementPage tab={0}>
      <ExtensionsMarketplacePage />
    </ExtensionManagementPage>
  ),
});
