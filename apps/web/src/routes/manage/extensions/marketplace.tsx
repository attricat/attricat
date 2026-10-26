import { createFileRoute } from '@tanstack/react-router';
import { extensionManagementTabIndex } from '../../../features/extensions/constants';
import { ExtensionManagementPage } from '../../../features/extensions/ExtensionManagementPage';
import { ExtensionsMarketplacePage } from '../../../features/extensions/ExtensionsMarketplacePage';

export const Route = createFileRoute('/manage/extensions/marketplace')({
  component: () => (
    <ExtensionManagementPage tab={extensionManagementTabIndex.marketplace}>
      <ExtensionsMarketplacePage />
    </ExtensionManagementPage>
  ),
});
