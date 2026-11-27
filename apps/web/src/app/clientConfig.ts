import { useQuery } from '@tanstack/react-query';
import { z } from 'zod';
import { request } from '../api/request';
import { supportedLanguages } from '../i18n';

export const clientConfigSchema = z.object({
  /** A language this build lacks counts as no preference. */
  default_language: z.enum(supportedLanguages).nullable().catch(null),
  /** The only workspace a single-workspace deployment signs in to. */
  sign_in_workspace: z.string().nullable(),
});

export type ClientConfig = z.infer<typeof clientConfigSchema>;

export const clientConfigQueryKey = ['client-config'] as const;

export const fetchClientConfig = () =>
  request('/api/client-config', clientConfigSchema);

/** Settles to `null` when the lookup fails, which leaves today's defaults. */
export const loadClientConfig = (): Promise<ClientConfig | null> =>
  fetchClientConfig().catch(() => null);

/** Seeded before the app renders, so this reads from the cache there. */
export const useClientConfig = () => {
  const query = useQuery({
    queryKey: clientConfigQueryKey,
    queryFn: fetchClientConfig,
    retry: false,
    staleTime: Infinity,
  });
  return { config: query.data ?? null, pending: query.isPending };
};
