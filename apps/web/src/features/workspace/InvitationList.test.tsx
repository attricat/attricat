// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../i18n';
import { InvitationList } from './InvitationList';
import type { Invitation } from './schemas';

const invitation = (
  email: string,
  overrides: Partial<Invitation> = {},
): Invitation => ({
  id: crypto.randomUUID(),
  invitee_email: email,
  inviter_user_id: crypto.randomUUID(),
  inviter_email: 'owner@example.test',
  role_id: crypto.randomUUID(),
  role_code: 'viewer',
  scope_type: 'workspace',
  scope_target_id: crypto.randomUUID(),
  expires_at: '2099-01-01T12:00:00Z',
  accepted_at: null,
  accepted_by_user_id: null,
  revoked_at: null,
  created_at: '2026-01-01T12:00:00Z',
  ...overrides,
});

describe('InvitationList', () => {
  it('shows each invitation state and offers Revoke only for open ones', () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <InvitationList
          invitations={[
            invitation('open@example.test'),
            invitation('accepted@example.test', {
              accepted_at: '2026-02-01T12:00:00Z',
            }),
            invitation('revoked@example.test', {
              revoked_at: '2026-02-01T12:00:00Z',
            }),
            invitation('expired@example.test', {
              expires_at: '2026-01-02T12:00:00Z',
            }),
          ]}
          onChanged={() => undefined}
          onError={() => undefined}
        />
      </QueryClientProvider>,
    );
    const row = (email: string) => screen.getByText(email).closest('li')!;
    expect(
      within(row('open@example.test')).getByText(/viewer · expires/),
    ).toBeTruthy();
    expect(
      within(row('open@example.test')).getByRole('button', { name: 'Revoke' }),
    ).toBeTruthy();
    for (const [email, state] of [
      ['accepted@example.test', /viewer · accepted/],
      ['revoked@example.test', /viewer · revoked/],
      ['expired@example.test', /viewer · expired/],
    ] as const) {
      expect(within(row(email)).getByText(state)).toBeTruthy();
      expect(within(row(email)).queryByRole('button')).toBeNull();
    }
  });
});
