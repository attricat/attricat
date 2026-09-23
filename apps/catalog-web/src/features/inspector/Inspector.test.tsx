// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { Inspector } from './Inspector';

vi.mock('../auth/api', () => ({
  currentSession: vi.fn(() => Promise.resolve(null)),
}));
vi.mock('./api', () => ({ getApiHealth: vi.fn(() => Promise.resolve({})) }));

describe('Inspector', () => {
  it('connects each tab to the active panel by stable accessible IDs', () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <Inspector />
      </QueryClientProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Open Inspector' }));
    const tabs = screen.getAllByRole('tab');
    for (const tab of tabs) {
      expect(tab.id).toBeTruthy();
      expect(tab.getAttribute('aria-controls')).toBeTruthy();
    }
    fireEvent.click(tabs[1]);
    const panel = screen.getByRole('tabpanel');
    expect(panel.id).toBe(tabs[1].getAttribute('aria-controls'));
    expect(panel.getAttribute('aria-labelledby')).toBe(tabs[1].id);
    expect(within(panel).getByText(/timing/i)).toBeTruthy();
  });
});
