import { createFileRoute } from '@tanstack/react-router';
import { extensionManagementTabIndex } from '../../../features/extensions/constants';
import { ExtensionManagementPage } from '../../../features/extensions/ExtensionManagementPage';
import { ExtensionSecretsPage } from '../../../features/extensions/ExtensionSecretsPage';

export const Route = createFileRoute('/manage/extensions/secrets')({
  component: () => (
    <ExtensionManagementPage tab={extensionManagementTabIndex.secrets}>
      <ExtensionSecretsPage />
    </ExtensionManagementPage>
  ),
});
