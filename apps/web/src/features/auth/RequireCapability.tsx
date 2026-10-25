import { Alert } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { useAccess, type Capability } from './useCan';
import { sessionOptions } from './queryOptions';
import { accessTargetKinds, type AccessTargetKind } from './capabilities';

/**
 * Renders a page only for callers holding `capability`, workspace-wide or on
 * `target` when one is named, and a refusal for a loaded session without it.
 * Session errors fall through to the page, whose requests the server still
 * authorizes.
 */
export const RequireCapability = ({
  capability,
  children,
  target,
}: {
  capability: Capability;
  children: ReactNode;
  target?: { kind: AccessTargetKind; id: string };
}) => {
  const { t } = useTranslation();
  const session = useQuery(sessionOptions());
  const access = useAccess(
    capability,
    target?.kind ?? accessTargetKinds.record,
    target ? [target.id] : [],
  );
  const allowed = target
    ? access.can(target.id)
    : session.data?.capabilities?.[capability] === true;
  if (session.isPending || (target && access.isPending)) return null;
  if (session.data && !allowed)
    return (
      <PageContainer>
        <Alert severity="error">{t(`auth.notAuthorized.${capability}`)}</Alert>
      </PageContainer>
    );
  return children;
};
