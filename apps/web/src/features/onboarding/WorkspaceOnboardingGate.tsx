import { CircularProgress } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { lazy, Suspense, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { sessionOptions } from '../auth/queryOptions';
import { blueprintCatalogueOptions } from '../blueprints/queryOptions';

// Most sessions never render onboarding, so keep it out of the startup bundle.
const WorkspaceOnboardingPage = lazy(() =>
  import('./WorkspaceOnboardingPage').then((module) => ({
    default: module.WorkspaceOnboardingPage,
  })),
);

// Shows workspace onboarding instead of `children` until the first blueprint,
// including a draft, exists. Load failures fall through to `children` so the
// home page never becomes unreachable.
export const WorkspaceOnboardingGate = ({
  children,
  disabled = false,
}: {
  children: ReactNode;
  disabled?: boolean;
}) => {
  const { t } = useTranslation();
  const session = useQuery({
    ...sessionOptions(),
    enabled: !disabled,
  });
  const blueprints = useQuery({
    ...blueprintCatalogueOptions(),
    enabled: !disabled,
  });
  if (disabled) return children;
  const loading = <CircularProgress aria-label={t('onboarding.loading')} />;
  if (session.isPending || blueprints.isPending) return loading;
  if (session.isError || blueprints.isError || blueprints.data.length)
    return children;
  return (
    <Suspense fallback={loading}>
      <WorkspaceOnboardingPage capabilities={session.data?.capabilities} />
    </Suspense>
  );
};
