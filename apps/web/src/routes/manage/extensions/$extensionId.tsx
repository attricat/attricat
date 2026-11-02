import { createFileRoute } from '@tanstack/react-router';
import { useEffect } from 'react';
import { z } from 'zod';
import {
  browserAuthorizationSearchSchema,
  showBrowserAuthorizationNotice,
} from '../../../features/extension-exports/browserAuthorizationNotice';
import { installedExtensionTabs } from '../../../features/extensions/constants';
import { InstalledExtensionPage } from '../../../features/extensions/InstalledExtensionPage';

const Page = () => {
  const { authorization, connection, tab } = Route.useSearch();
  const navigate = Route.useNavigate();
  // Shown once when the OAuth callback returns here, then cleared so a
  // reload does not repeat it; the connection it names is on its own tab.
  useEffect(() => {
    if (!authorization) return;
    showBrowserAuthorizationNotice(authorization, connection);
    void navigate({
      search: { tab: installedExtensionTabs.connections },
      replace: true,
    });
  }, [authorization, connection, navigate]);
  return (
    <InstalledExtensionPage
      extensionId={Route.useParams().extensionId}
      tab={tab}
    />
  );
};
export const Route = createFileRoute('/manage/extensions/$extensionId')({
  validateSearch: browserAuthorizationSearchSchema.extend({
    tab: z.enum(installedExtensionTabs).optional().catch(undefined),
  }),
  component: Page,
});
