import { createFileRoute } from '@tanstack/react-router';
import { ExtensionRunsPage } from '../../../features/extension-runs/ExtensionRunsPage';
import { ProfileTabs } from '../../../features/profile/ProfileTabs';
import { profileTabIndex } from '../../../features/profile/constants';

export const Route = createFileRoute('/profile/extension-runs/')({
  component: () => (
    <ProfileTabs tab={profileTabIndex.extensionRuns}>
      <ExtensionRunsPage />
    </ProfileTabs>
  ),
});
