import { createFileRoute } from '@tanstack/react-router';
import { capabilities } from '../../../features/auth/capabilities';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { SideloadExtensionPage } from '../../../features/extensions/SideloadExtensionPage';

export const Route = createFileRoute('/manage/extensions/sideload')({
  component: () => (
    <RequireCapability capability={capabilities.extensionsManage}>
      <SideloadExtensionPage />
    </RequireCapability>
  ),
});
