import { createFileRoute } from '@tanstack/react-router';
import { ExtensionRunDetailPage } from '../../../features/extension-runs/ExtensionRunDetailPage';
import { ProfileTabs } from '../../../features/profile/ProfileTabs';
import { profileTabIndex } from '../../../features/profile/constants';

export const Route = createFileRoute('/profile/extension-runs/$runId')({
  component: function ExtensionRunRoute() {
    return (
      <ProfileTabs tab={profileTabIndex.extensionRuns}>
        <ExtensionRunDetailPage runId={Route.useParams().runId} />
      </ProfileTabs>
    );
  },
});
