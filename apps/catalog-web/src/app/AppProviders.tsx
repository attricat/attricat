import {
  CssBaseline,
  ThemeProvider,
  createTheme,
  useMediaQuery,
} from '@mui/material';
import { plPL } from '@mui/material/locale';
import { RouterProvider } from '@tanstack/react-router';
import { LucideProvider } from 'lucide-react';
import { lazy, Suspense, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ToastProvider } from '../components/ToastProvider';
import { inspectorEnabled } from '../features/inspector/enabled';
import { router } from './router';
import { makeTheme } from './theme';
import { useColorMode } from './colorMode';

// LucideProvider types size as a number but forwards it to the SVG width and
// height, so icons can follow their MUI container font size like SvgIcon.
const lucideIconSize = '1em' as unknown as number;

// Loaded on demand so browsers that never enable it do not download it.
const Inspector = lazy(() =>
  import('../features/inspector/Inspector').then(({ Inspector }) => ({
    default: Inspector,
  })),
);

export const AppProviders = () => {
  const { i18n } = useTranslation();
  const language = i18n.resolvedLanguage === 'pl' ? 'pl' : 'en';
  const prefersDark = useMediaQuery('(prefers-color-scheme: dark)');
  const preference = useColorMode((state) => state.preference);
  const mode = preference ?? (prefersDark ? 'dark' : 'light');
  const localizedTheme = useMemo(
    () =>
      language === 'pl' ? createTheme(makeTheme(mode), plPL) : makeTheme(mode),
    [language, mode],
  );

  useEffect(() => {
    document.documentElement.lang = language;
    document.documentElement.style.colorScheme = mode;
  }, [language, mode]);

  return (
    <ThemeProvider theme={localizedTheme}>
      <CssBaseline />
      <LucideProvider size={lucideIconSize}>
        <ToastProvider>
          <RouterProvider router={router} />
          {inspectorEnabled && (
            <Suspense fallback={null}>
              <Inspector />
            </Suspense>
          )}
        </ToastProvider>
      </LucideProvider>
    </ThemeProvider>
  );
};
