// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { type Session, updatePreferences } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { browserTimeZone } from '../../time/instantFormat';
import { TimeZonePreference } from './TimeZonePreference';

vi.mock('../auth/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../auth/api')>()),
  updatePreferences: vi.fn(),
}));

const session = (time_zone: string | null): Session => ({
  user_id: '123e4567-e89b-12d3-a456-426614174000',
  display_name: null,
  email: 'owner@example.test',
  time_zone,
  avatar: null,
  workspace_id: '123e4567-e89b-12d3-a456-426614174001',
  login_identifier: 'default.local',
});

const renderPreference = (timeZone: string | null) => {
  const client = new QueryClient();
  client.setQueryData(authQueryKeys.session(), session(timeZone));
  render(
    <QueryClientProvider client={client}>
      <TimeZonePreference disabled={false} />
    </QueryClientProvider>,
  );
  return client;
};

describe('TimeZonePreference', () => {
  it('shows the automatic option when no zone is stored', () => {
    renderPreference(null);
    expect(
      (screen.getByRole('combobox', { name: 'Time zone' }) as HTMLInputElement)
        .value,
    ).toBe(`Automatic (browser: ${browserTimeZone()})`);
  });

  it('stores a chosen zone and updates the session cache', async () => {
    vi.mocked(updatePreferences).mockResolvedValue(session('Asia/Tokyo'));
    const client = renderPreference(null);
    const input = screen.getByRole('combobox', { name: 'Time zone' });
    await userEvent.clear(input);
    await userEvent.type(input, 'Asia/Tokyo');
    await userEvent.click(
      await screen.findByRole('option', { name: /^Asia\/Tokyo / }),
    );
    expect(updatePreferences).toHaveBeenCalledWith({ time_zone: 'Asia/Tokyo' });
    await waitFor(() =>
      expect(
        client.getQueryData<Session>(authQueryKeys.session())?.time_zone,
      ).toBe('Asia/Tokyo'),
    );
    expect(
      (screen.getByRole('combobox', { name: 'Time zone' }) as HTMLInputElement)
        .value,
    ).toMatch(/^Asia\/Tokyo /);
  });

  it('clears the stored zone with the automatic option', async () => {
    vi.mocked(updatePreferences).mockResolvedValue(session(null));
    renderPreference('Asia/Tokyo');
    const input = screen.getByRole('combobox', { name: 'Time zone' });
    await userEvent.clear(input);
    await userEvent.type(input, 'Automatic');
    await userEvent.click(
      await screen.findByRole('option', { name: /^Automatic/ }),
    );
    expect(updatePreferences).toHaveBeenCalledWith({ time_zone: null });
  });

  it('reports a failed save', async () => {
    vi.mocked(updatePreferences).mockRejectedValue(
      new Error('unknown IANA time zone'),
    );
    renderPreference(null);
    const input = screen.getByRole('combobox', { name: 'Time zone' });
    await userEvent.clear(input);
    await userEvent.type(input, 'Asia/Tokyo');
    await userEvent.click(
      await screen.findByRole('option', { name: /^Asia\/Tokyo / }),
    );
    expect(
      await screen.findByText(
        'Could not save your time zone: unknown IANA time zone',
      ),
    ).toBeTruthy();
  });
});
