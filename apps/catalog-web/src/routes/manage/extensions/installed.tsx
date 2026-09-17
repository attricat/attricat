import { createFileRoute } from '@tanstack/react-router';
import { ExtensionManagementPage } from '../../../features/extensions/ExtensionManagementPage';
import { ExtensionsInstalledPage } from '../../../features/extensions/ExtensionsInstalledPage';

export const Route = createFileRoute('/manage/extensions/installed')({
  component: () => (
    <ExtensionManagementPage tab={1}>
      <ExtensionsInstalledPage />
    </ExtensionManagementPage>
  ),
});
