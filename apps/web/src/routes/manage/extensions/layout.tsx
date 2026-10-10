import { createFileRoute } from '@tanstack/react-router';
import { ExtensionManagementPage } from '../../../features/extensions/ExtensionManagementPage';
import { ExtensionsLayoutPage } from '../../../features/extensions/ExtensionsLayoutPage';

export const Route = createFileRoute('/manage/extensions/layout')({
  component: () => (
    <ExtensionManagementPage tab={2}>
      <ExtensionsLayoutPage />
    </ExtensionManagementPage>
  ),
});
