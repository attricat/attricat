import { createFileRoute } from '@tanstack/react-router';
import { extensionManagementTabIndex } from '../../../features/extensions/constants';
import { ExtensionManagementPage } from '../../../features/extensions/ExtensionManagementPage';
import { ExtensionsLayoutPage } from '../../../features/extensions/ExtensionsLayoutPage';

export const Route = createFileRoute('/manage/extensions/layout')({
  component: () => (
    <ExtensionManagementPage tab={extensionManagementTabIndex.layout}>
      <ExtensionsLayoutPage />
    </ExtensionManagementPage>
  ),
});
