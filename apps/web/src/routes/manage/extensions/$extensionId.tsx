import { createFileRoute } from '@tanstack/react-router';
import { useEffect } from 'react';
import {
  browserAuthorizationSearchSchema,
  showBrowserAuthorizationNotice,
} from '../../../features/extension-exports/browserAuthorizationNotice';
import { InstalledExtensionPage } from '../../../features/extensions/InstalledExtensionPage';

const Page = () => {
  const { authorization, connection } = Route.useSearch();
  const navigate = Route.useNavigate();
  // Shown once when the OAuth callback returns here, then cleared so a
  // reload does not repeat it.
  useEffect(() => {
    if (!authorization) return;
    showBrowserAuthorizationNotice(authorization, connection);
    void navigate({ search: {}, replace: true });
  }, [authorization, connection, navigate]);
  return <InstalledExtensionPage extensionId={Route.useParams().extensionId} />;
};
export const Route = createFileRoute('/manage/extensions/$extensionId')({
  validateSearch: browserAuthorizationSearchSchema,
  component: Page,
});
