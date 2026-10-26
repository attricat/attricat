// @vitest-environment jsdom
import type { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import { invalidInputErrorCode } from '../../api/errors';
import { ApiRequestError } from '../../api/request';
import { checkStatuses, useDefinitionCheck } from './useDefinitionCheck';

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={new QueryClient()}>
    {children}
  </QueryClientProvider>
);

describe('useDefinitionCheck', () => {
  it('keeps the untranslated server message to find where an error points', async () => {
    await i18n.changeLanguage('en');
    const { result } = renderHook(
      () =>
        useDefinitionCheck({
          definition: 'code = "renamed"',
          kind: 'blueprint',
          validate: () =>
            Promise.reject(
              new ApiRequestError(
                422,
                'code is immutable',
                invalidInputErrorCode,
              ),
            ),
        }),
      { wrapper },
    );
    await waitFor(() =>
      expect(result.current.status).toBe(checkStatuses.invalid),
    );
    expect(result.current.error).toBe(
      'The request was rejected: code is immutable',
    );
    expect(result.current.serverMessage).toBe('code is immutable');
  });
});
