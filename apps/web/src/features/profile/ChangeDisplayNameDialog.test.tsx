// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { updateDisplayName } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { ChangeDisplayNameDialog } from './ChangeDisplayNameDialog';

vi.mock('../auth/api', () => ({ updateDisplayName: vi.fn() }));

const renderDialog = () => {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });
  const onClose = vi.fn();
  render(
    <QueryClientProvider client={client}>
      <ChangeDisplayNameDialog initialName="Old Name" onClose={onClose} />
    </QueryClientProvider>,
  );
  return { client, onClose };
};

const typeName = async (
  user: ReturnType<typeof userEvent.setup>,
  value: string,
) => {
  const input = screen.getByRole('textbox', { name: 'Display name' });
  await user.clear(input);
  if (value) await user.type(input, value);
};

describe('ChangeDisplayNameDialog', () => {
  it('saves a valid name and updates the cached session', async () => {
    const user = userEvent.setup();
    const { client, onClose } = renderDialog();
    const session = { display_name: 'Ada Lovelace 2' };
    vi.mocked(updateDisplayName).mockResolvedValue(session as never);
    await typeName(user, 'Ada Lovelace 2');
    await user.click(screen.getByRole('button', { name: 'Save name' }));
    await waitFor(() =>
      expect(updateDisplayName).toHaveBeenCalledWith('Ada Lovelace 2'),
    );
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect(client.getQueryData(authQueryKeys.session())).toBe(session);
  });

  it.each([
    ['A', 'Use at least 2 characters.'],
    ['a'.repeat(65), 'Use at most 64 characters.'],
    ['Ada-Lovelace', 'Use only letters, digits, and spaces.'],
    [' Ada', 'Remove spaces from the start and end.'],
    ['Ada ', 'Remove spaces from the start and end.'],
  ])('rejects %j without making a request', async (value, message) => {
    const user = userEvent.setup();
    renderDialog();
    await typeName(user, value);
    await user.click(screen.getByRole('button', { name: 'Save name' }));
    expect(await screen.findByText(message)).toBeTruthy();
    expect(updateDisplayName).not.toHaveBeenCalled();
  });

  it('shows API errors and keeps the dialog open', async () => {
    const user = userEvent.setup();
    vi.mocked(updateDisplayName).mockRejectedValue(new Error('Server said no'));
    const { onClose } = renderDialog();
    await user.click(screen.getByRole('button', { name: 'Save name' }));
    expect(await screen.findByText('Server said no')).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});
