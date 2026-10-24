import { createFileRoute, redirect } from '@tanstack/react-router';
import {
  browserAuthorizationSearchSchema,
  showBrowserAuthorizationNotice,
} from '../../../features/extension-exports/browserAuthorizationNotice';

export const Route = createFileRoute('/manage/extensions/')({
  // The OAuth callback lands here when it cannot tell which extension an
  // authorization belonged to.
  validateSearch: browserAuthorizationSearchSchema,
  beforeLoad: ({ search }) => {
    if (search.authorization)
      showBrowserAuthorizationNotice(search.authorization);
    throw redirect({ to: '/manage/extensions/marketplace' });
  },
});
