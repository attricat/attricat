// @vitest-environment jsdom
import type { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
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

  it('marks the result stale until the definition is checked in a new scope', async () => {
    let resolveSecond: (value: string) => void = () => {};
    const validate = vi
      .fn<(definition: string) => Promise<string>>()
      .mockResolvedValueOnce('first scope')
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveSecond = resolve;
        }),
      );
    const { result, rerender } = renderHook(
      ({ scope }) =>
        useDefinitionCheck({
          definition: 'code = "title"',
          kind: 'rule',
          scope,
          validate,
        }),
      { initialProps: { scope: 'products@1' }, wrapper },
    );
    await waitFor(() => expect(result.current.result).toBe('first scope'));
    expect(result.current.stale).toBe(false);

    rerender({ scope: 'products@2' });
    await waitFor(() => expect(validate).toHaveBeenCalledTimes(2));
    expect(result.current.result).toBe('first scope');
    expect(result.current.stale).toBe(true);

    resolveSecond('second scope');
    await waitFor(() => expect(result.current.result).toBe('second scope'));
    expect(result.current.stale).toBe(false);
  });
});
