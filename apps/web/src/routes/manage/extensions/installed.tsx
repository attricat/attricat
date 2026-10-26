import { createFileRoute } from '@tanstack/react-router';
import { extensionManagementTabIndex } from '../../../features/extensions/constants';
import { ExtensionManagementPage } from '../../../features/extensions/ExtensionManagementPage';
import { ExtensionsInstalledPage } from '../../../features/extensions/ExtensionsInstalledPage';

export const Route = createFileRoute('/manage/extensions/installed')({
  component: () => (
    <ExtensionManagementPage tab={extensionManagementTabIndex.installed}>
      <ExtensionsInstalledPage />
    </ExtensionManagementPage>
  ),
});
