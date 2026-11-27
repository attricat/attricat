// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { changePassword } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { toast } from '../../components/toast';
import { ChangePasswordDialog } from './ChangePasswordDialog';

vi.mock('../auth/api', () => ({ changePassword: vi.fn() }));
vi.mock('../../components/toast', () => ({ toast: { success: vi.fn() } }));

const renderDialog = () => {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });
  const onClose = vi.fn();
  render(
    <QueryClientProvider client={client}>
      <ChangePasswordDialog onClose={onClose} />
    </QueryClientProvider>,
  );
  return { client, onClose };
};

const fill = async (
  user: ReturnType<typeof userEvent.setup>,
  values: { current: string; next: string; confirm: string },
) => {
  await user.type(screen.getByLabelText(/Current password/), values.current);
  await user.type(screen.getByLabelText(/^New password/), values.next);
  await user.type(
    screen.getByLabelText(/Confirm new password/),
    values.confirm,
  );
  await user.click(screen.getByRole('button', { name: 'Change password' }));
};

describe('ChangePasswordDialog', () => {
  afterEach(() => vi.clearAllMocks());

  it('changes the password and keeps the replacement session', async () => {
    const user = userEvent.setup();
    const { client, onClose } = renderDialog();
    const session = { email: 'ada@example.test' };
    vi.mocked(changePassword).mockResolvedValue(session as never);
    await fill(user, {
      current: 'old password 1!',
      next: 'new password 2!',
      confirm: 'new password 2!',
    });
    await waitFor(() =>
      expect(changePassword).toHaveBeenCalledWith(
        'old password 1!',
        'new password 2!',
      ),
    );
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect(client.getQueryData(authQueryKeys.session())).toBe(session);
    expect(toast.success).toHaveBeenCalledWith(
      'Password changed. Other browsers were signed out.',
    );
  });

  it('asks for matching new passwords before sending anything', async () => {
    const user = userEvent.setup();
    renderDialog();
    await fill(user, {
      current: 'old password 1!',
      next: 'new password 2!',
      confirm: 'new password 3!',
    });
    expect(await screen.findByText('Passwords do not match.')).toBeTruthy();
    expect(changePassword).not.toHaveBeenCalled();
  });

  it('explains a wrong current password and stays open', async () => {
    const user = userEvent.setup();
    const { onClose } = renderDialog();
    vi.mocked(changePassword).mockRejectedValue(
      new ApiRequestError(422, 'incorrect', 'current_password_incorrect'),
    );
    await fill(user, {
      current: 'wrong password 1!',
      next: 'new password 2!',
      confirm: 'new password 2!',
    });
    expect(
      await screen.findByText('The current password is incorrect.'),
    ).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});
