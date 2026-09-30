import { createFileRoute } from '@tanstack/react-router';
import { ProfilePage } from '../../features/profile/ProfilePage';
import { ProfileTabs } from '../../features/profile/ProfileTabs';
import { profileTabIndex } from '../../features/profile/constants';

export const Route = createFileRoute('/profile/')({
  component: () => (
    <ProfileTabs tab={profileTabIndex.account}>
      <ProfilePage />
    </ProfileTabs>
  ),
});
