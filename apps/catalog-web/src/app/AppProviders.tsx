import { CssBaseline, ThemeProvider, createTheme } from '@mui/material';
import { plPL } from '@mui/material/locale';
import { RouterProvider } from '@tanstack/react-router';
import { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Inspector } from '../features/inspector/Inspector';
import { router } from './router';
import { theme } from './theme';

export const AppProviders = () => {
  const { i18n } = useTranslation();
  const language = i18n.resolvedLanguage === 'pl' ? 'pl' : 'en';
  const localizedTheme = useMemo(
    () => (language === 'pl' ? createTheme(theme, plPL) : createTheme(theme)),
    [language],
  );

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  return (
    <ThemeProvider theme={localizedTheme}>
      <CssBaseline />
      <RouterProvider router={router} />
      {import.meta.env.DEV && __CATALOG_DEVTOOLS__ && <Inspector />}
    </ThemeProvider>
  );
};
