// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { removeAvatar, uploadAvatar } from '../auth/api';
import { AvatarControls } from './AvatarControls';
import { avatarMaxBytes } from './constants';

vi.mock('../auth/api', () => ({
  removeAvatar: vi.fn(),
  uploadAvatar: vi.fn(),
}));

const renderControls = (hasAvatar = false) => {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });
  const onProgress = vi.fn();
  render(
    <QueryClientProvider client={client}>
      <AvatarControls
        disabled={false}
        hasAvatar={hasAvatar}
        onProgress={onProgress}
      />
    </QueryClientProvider>,
  );
  return { onProgress };
};

// `applyAccept: false` lets the test offer files the picker would filter out.
const setup = () => userEvent.setup({ applyAccept: false });

describe('AvatarControls', () => {
  it('uploads a PNG and reports progress', async () => {
    const user = setup();
    const { onProgress } = renderControls();
    vi.mocked(uploadAvatar).mockResolvedValue({
      file_id: '123e4567-e89b-12d3-a456-426614174000',
      status: 'queued',
    });
    const file = new File(['png'], 'me.png', { type: 'image/png' });
    await user.upload(screen.getByLabelText('Profile photo'), file);
    await waitFor(() =>
      expect(uploadAvatar).toHaveBeenCalledWith(file, onProgress),
    );
    expect(onProgress).toHaveBeenCalledWith(0);
    await waitFor(() => expect(onProgress).toHaveBeenLastCalledWith(null));
  });

  const oversized = () => {
    const file = new File(['jpg'], 'big.jpg', { type: 'image/jpeg' });
    Object.defineProperty(file, 'size', { value: avatarMaxBytes + 1 });
    return file;
  };

  it.each([
    [
      'a GIF',
      () => new File(['gif'], 'me.gif', { type: 'image/gif' }),
      'Choose a PNG or JPEG image.',
    ],
    ['an oversized image', oversized, 'Choose an image of 10 MB or less.'],
  ])('rejects %s without uploading', async (_, file, message) => {
    const user = setup();
    renderControls();
    await user.upload(screen.getByLabelText('Profile photo'), file());
    expect(await screen.findByText(message)).toBeTruthy();
    expect(uploadAvatar).not.toHaveBeenCalled();
  });

  it('removes an existing avatar', async () => {
    const user = setup();
    renderControls(true);
    vi.mocked(removeAvatar).mockResolvedValue();
    await user.click(screen.getByRole('button', { name: 'Remove photo' }));
    await waitFor(() => expect(removeAvatar).toHaveBeenCalledOnce());
  });
});
