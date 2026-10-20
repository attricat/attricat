import {
  MutationCache,
  QueryClient,
  QueryClientProvider,
} from '@tanstack/react-query';
import { createRoot } from 'react-dom/client';
import { AppProviders } from './app/AppProviders';
import { toast } from './components/toast';
import i18n, { i18nReady } from './i18n';
import { router } from './app/router';
import { authQueryKeys } from './features/auth/queryKeys';
import { contextQueryKeys } from './features/contexts/queryKeys';
import { recordQueryKeys } from './features/records/queryKeys';
import { reusableAttributeQueryKeys } from './features/reusable-attributes/queryKeys';

const queryClient = new QueryClient({
  mutationCache: new MutationCache({
    onSuccess: (_, __, ___, mutation) => {
      if (mutation.options.meta?.toast === false) return;
      toast.success(i18n.t('common.actionCompleted'));
    },
  }),
});

// Reads that many views mount at once, such as the Explorer and its record
// panel, would otherwise be fetched again by every component that mounts
// them. Changes made in the app invalidate them; changes made elsewhere show
// when the window regains focus.
for (const queryKey of [
  authQueryKeys.session(),
  contextQueryKeys.all(),
  recordQueryKeys.blueprints(),
  recordQueryKeys.blueprintsByCode(),
  reusableAttributeQueryKeys.root(),
  reusableAttributeQueryKeys.groups(),
])
  queryClient.setQueryDefaults(queryKey, {
    staleTime: Infinity,
    refetchOnWindowFocus: 'always',
  });

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}

// Do not render untranslated keys while the selected locale chunk loads.
void i18nReady.then(() => {
  createRoot(document.getElementById('root')!).render(
    <QueryClientProvider client={queryClient}>
      <AppProviders />
    </QueryClientProvider>,
  );
});
