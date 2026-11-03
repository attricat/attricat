// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { startSingleSignOn } from './api';
import { SingleSignOnButton } from './SingleSignOnButton';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  startSingleSignOn: vi.fn(),
}));
vi.mock('./returnLocation', () => ({ leaveForProvider: vi.fn() }));

it('works again when the browser restores the page after leaving', async () => {
  vi.mocked(startSingleSignOn).mockResolvedValue({
    authorization_url: 'https://idp.example.com/authorize',
  } as never);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <SingleSignOnButton displayName="Acme" loginIdentifier="acme" />
    </QueryClientProvider>,
  );
  const button = screen.getByRole('button');

  await userEvent.setup().click(button);
  await waitFor(() => expect(button).toHaveProperty('disabled', true));

  act(() => {
    window.dispatchEvent(
      new PageTransitionEvent('pageshow', { persisted: true }),
    );
  });
  await waitFor(() => expect(button).toHaveProperty('disabled', false));
});
