import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { CssBaseline, ThemeProvider } from '@mui/material';
import { createRoot } from 'react-dom/client';
import { router } from './app/router';
import { theme } from './app/theme';
import { Inspector } from './features/inspector/Inspector';

const queryClient = new QueryClient();
declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}

createRoot(document.getElementById('root')!).render(
  <QueryClientProvider client={queryClient}>
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <RouterProvider router={router} />
      {import.meta.env.DEV && __CATALOG_DEVTOOLS__ && <Inspector />}
    </ThemeProvider>
  </QueryClientProvider>,
);
