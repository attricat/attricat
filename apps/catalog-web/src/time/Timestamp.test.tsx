// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { Trans } from 'react-i18next';
import { afterEach, describe, expect, it } from 'vitest';
import i18n from '../i18n';
import type { Session } from '../features/auth/api';
import { authQueryKeys } from '../features/auth/queryKeys';
import {
  browserTimeZone,
  formatInstant,
  isBrowserTimeZone,
} from './instantFormat';
import { Timestamp } from './Timestamp';

const instant = '2026-09-29T14:03:12Z';
const session = (time_zone: string | null): Session => ({
  user_id: '123e4567-e89b-12d3-a456-426614174000',
  display_name: null,
  email: 'owner@example.test',
  time_zone,
  avatar: null,
  workspace_id: '123e4567-e89b-12d3-a456-426614174001',
  login_identifier: 'default.local',
});
// A zone that differs from the test runner's zone in either case.
const preferredZone = isBrowserTimeZone('Asia/Tokyo')
  ? 'America/New_York'
  : 'Asia/Tokyo';

const renderWithSession = (
  timeZone: string | null,
  children: ReactNode,
  client = new QueryClient(),
) => {
  client.setQueryData(authQueryKeys.session(), session(timeZone));
  return {
    client,
    ...render(
      <QueryClientProvider client={client}>{children}</QueryClientProvider>,
    ),
  };
};

afterEach(async () => {
  await i18n.changeLanguage('en');
});

describe('Timestamp', () => {
  it('renders the instant in the preferred zone with a zone label', () => {
    renderWithSession(preferredZone, <Timestamp value={instant} />);
    const time = screen.getByText(
      formatInstant(instant, {
        locale: 'en',
        timeZone: preferredZone,
        showTimeZone: true,
      }),
    );
    expect(time.tagName).toBe('TIME');
    expect(time.getAttribute('datetime')).toBe('2026-09-29T14:03:12.000Z');
  });

  it('follows the browser zone without a label when no zone is stored', () => {
    renderWithSession(null, <Timestamp value={instant} />);
    expect(
      screen.getByText(
        formatInstant(instant, { locale: 'en', timeZone: browserTimeZone() }),
      ),
    ).toBeTruthy();
  });

  it('re-renders when the preference changes', async () => {
    const { client } = renderWithSession(null, <Timestamp value={instant} />);
    act(() =>
      client.setQueryData(authQueryKeys.session(), session(preferredZone)),
    );
    expect(
      await screen.findByText(
        formatInstant(instant, {
          locale: 'en',
          timeZone: preferredZone,
          showTimeZone: true,
        }),
      ),
    ).toBeTruthy();
  });

  it('is reachable by keyboard and shows the exact UTC time', async () => {
    renderWithSession(preferredZone, <Timestamp value={instant} />);
    await userEvent.tab();
    const time = screen.getByText(/2026/);
    expect(document.activeElement).toBe(time);
    // jsdom cannot report :focus-visible, which MUI needs to open on focus.
    await userEvent.hover(time);
    const tooltip = await screen.findByRole('tooltip');
    expect(tooltip.textContent).toBe('2026-09-29 14:03:12 UTC');
    expect(time.getAttribute('aria-describedby')).toBe(tooltip.id);
  });

  it('is not a separate tab stop inside interactive content', () => {
    renderWithSession(null, <Timestamp focusable={false} value={instant} />);
    expect(screen.getByText(/2026/).hasAttribute('tabindex')).toBe(false);
  });

  it('renders the fallback when there is no value', () => {
    renderWithSession(null, <Timestamp fallback="Never" value={null} />);
    expect(screen.getByText('Never')).toBeTruthy();
  });

  it('can be embedded in a translated sentence', async () => {
    await i18n.changeLanguage('pl');
    renderWithSession(
      'UTC',
      <Trans
        components={{ timestamp: <Timestamp value={instant} /> }}
        i18nKey="agents.updatedAt"
      />,
    );
    const expected = formatInstant(instant, {
      locale: 'pl',
      timeZone: 'UTC',
      showTimeZone: !isBrowserTimeZone('UTC'),
    });
    expect(screen.getByText(expected).parentElement?.textContent).toBe(
      `Zaktualizowano ${expected}`,
    );
  });
});
