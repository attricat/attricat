import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { sessionOptions } from '../auth/queryOptions';

/**
 * Why agents cannot run, for a tooltip on the disabled control: set only once
 * the session says this deployment has no agent provider.
 */
export const useAgentsUnavailableReason = () => {
  const { t } = useTranslation();
  const { data: unavailable } = useQuery({
    ...sessionOptions(),
    select: (session) => session?.agents_configured === false,
  });
  return unavailable ? t('agents.notConfigured') : undefined;
};
