import { createFileRoute } from '@tanstack/react-router';
import { PersonalTokensPage } from '../../../features/profile/PersonalTokensPage';
import { ProfileTabs } from '../../../features/profile/ProfileTabs';
import { profileTabIndex } from '../../../features/profile/constants';

export const Route = createFileRoute('/profile/personal-access-tokens/')({
  component: () => (
    <ProfileTabs tab={profileTabIndex.personalTokens}>
      <PersonalTokensPage />
    </ProfileTabs>
  ),
});
