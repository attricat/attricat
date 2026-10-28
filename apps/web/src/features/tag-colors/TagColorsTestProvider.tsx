import { ThemeProvider } from '@mui/material/styles';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState, type ReactNode } from 'react';
import { makeTheme } from '../../app/theme';
import { NEEDS_ATTENTION_SYSTEM_TAG } from '../records/constants';
import { tagColorQueryKeys } from './queryKeys';
import type { TagColors } from './schemas';

/** The colors tests render with: the Core default and no overrides. */
const testTagColors: TagColors = {
  defaults: { [NEEDS_ATTENTION_SYSTEM_TAG]: 'yellow' },
  overrides: {},
};

/**
 * Renders tag chips in the light theme with `colors` already loaded, so tests
 * need no request for them.
 */
export const TagColorsTestProvider = ({
  children,
  colors = testTagColors,
}: {
  children: ReactNode;
  colors?: TagColors;
}) => {
  const [client] = useState(
    () =>
      new QueryClient({
        defaultOptions: { queries: { retry: false, staleTime: Infinity } },
      }),
  );
  client.setQueryData(tagColorQueryKeys.all(), colors);
  return (
    <QueryClientProvider client={client}>
      <ThemeProvider theme={makeTheme('light')}>{children}</ThemeProvider>
    </QueryClientProvider>
  );
};
