import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { CssBaseline, ThemeProvider, createTheme } from '@mui/material'
import { createRoot } from 'react-dom/client'
import { router } from './router'

const queryClient = new QueryClient()
const theme = createTheme({
  palette: { background: { default: '#f7f7f5' }, primary: { main: '#8a3d1e' } },
  shape: { borderRadius: 4 },
  typography: { fontFamily: 'Inter, ui-sans-serif, system-ui, sans-serif' },
})

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

createRoot(document.getElementById('root')!).render(
  <QueryClientProvider client={queryClient}>
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <RouterProvider router={router} />
    </ThemeProvider>
  </QueryClientProvider>,
)
