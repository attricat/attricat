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

const queryClient = new QueryClient({
  mutationCache: new MutationCache({
    onSuccess: (_, __, ___, mutation) => {
      if (mutation.options.meta?.toast === false) return;
      toast.success(i18n.t('common.actionCompleted'));
    },
  }),
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
