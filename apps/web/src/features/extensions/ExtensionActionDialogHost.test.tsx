// @vitest-environment jsdom
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { useActionDialogStore } from './actionDialogStore';
import { ExtensionActionDialogHost } from './ExtensionActionDialogHost';
import { useExtensionRuntime } from './useExtensionRuntime';

vi.mock('@tanstack/react-router', () => ({
  useRouterState: () => '/records',
}));
vi.mock('./useExtensionRuntime', () => ({ useExtensionRuntime: vi.fn() }));
vi.mock('./ExtensionFrame', () => ({ ExtensionFrame: () => 'frame' }));

describe('ExtensionActionDialogHost', () => {
  it('offers a retry when the extension runtime fails to load', async () => {
    const refetch = vi.fn();
    vi.mocked(useExtensionRuntime).mockReturnValue({
      data: undefined,
      error: new ApiRequestError(503, 'Unavailable', 'service_unavailable'),
      isError: true,
      isFetching: false,
      isPending: false,
      refetch,
    } as never);
    render(<ExtensionActionDialogHost />);
    act(() =>
      useActionDialogStore.getState().open({
        extensionId: 'acme.tools',
        context: {
          record_ids: ['record'],
          selection_source: 'record',
        } as never,
      }),
    );

    await userEvent.click(
      await screen.findByRole('button', { name: 'Try again' }),
    );
    expect(refetch).toHaveBeenCalledOnce();
    expect(screen.queryByText(/could not be loaded/i)).toBeNull();
  });
});
