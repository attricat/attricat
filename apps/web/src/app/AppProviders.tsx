import { useActiveLanguage } from '../i18n/useActiveLanguage';
import {
  CssBaseline,
  ThemeProvider,
  createTheme,
  useMediaQuery,
} from '@mui/material';
import { plPL, type Localization } from '@mui/material/locale';
import { RouterProvider } from '@tanstack/react-router';
import { LucideProvider } from 'lucide-react';
import { lazy, Suspense, useEffect, useMemo } from 'react';
import { ToastProvider } from '../components/ToastProvider';
import { inspectorEnabled } from '../features/inspector/enabled';
import { router } from './router';
import { makeTheme } from './theme';
import { useColorMode } from './colorMode';
import { type SupportedLanguage } from '../i18n';

/** MUI's built-in component strings per language; English is MUI's default. */
const muiLocales: Record<SupportedLanguage, Localization | undefined> = {
  en: undefined,
  pl: plPL,
};

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
  const language = useActiveLanguage();
  const prefersDark = useMediaQuery('(prefers-color-scheme: dark)');
  const preference = useColorMode((state) => state.preference);
  const mode = preference ?? (prefersDark ? 'dark' : 'light');
  const localizedTheme = useMemo(() => {
    const locale = muiLocales[language];
    return locale ? createTheme(makeTheme(mode), locale) : makeTheme(mode);
  }, [language, mode]);

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
