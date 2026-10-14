import { Alert } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { currentSession } from './api';
import { authQueryKeys } from './queryKeys';
import { useCan, type Capability } from './useCan';

/**
 * Renders a page only for callers holding `capability`, and a refusal for a
 * loaded session without it. Session errors fall through to the page, whose
 * requests the server still authorizes.
 */
export const RequireCapability = ({
  capability,
  children,
}: {
  capability: Capability;
  children: ReactNode;
}) => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const allowed = useCan(capability);
  if (session.isPending) return null;
  if (session.data && !allowed)
    return (
      <PageContainer>
        <Alert severity="error">{t(`auth.notAuthorized.${capability}`)}</Alert>
      </PageContainer>
    );
  return children;
};
