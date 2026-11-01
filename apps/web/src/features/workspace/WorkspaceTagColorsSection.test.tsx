// @vitest-environment jsdom
import { ThemeProvider } from '@mui/material/styles';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { makeTheme } from '../../app/theme';
import '../../i18n';
import {
  getTagColors,
  updateTagColors,
  type TagColors,
} from '../tag-colors/api';
import { WorkspaceTagColorsSection } from './WorkspaceTagColorsSection';

vi.mock('../tag-colors/api', () => ({
  getTagColors: vi.fn(),
  updateTagColors: vi.fn(),
}));

const colors: TagColors = {
  defaults: { 'attricat.needs_attention': 'gray' },
  overrides: { reviewed: 'blue' },
};

const renderSection = () =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ThemeProvider theme={makeTheme('light')}>
        <WorkspaceTagColorsSection canManage />
      </ThemeProvider>
    </QueryClientProvider>,
  );

describe('WorkspaceTagColorsSection', () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it.each(['attricat.needs_attention', ' reviewed '])(
    'keeps the color of %s rather than replacing it',
    async (tag) => {
      vi.mocked(getTagColors).mockResolvedValue(colors);
      renderSection();
      const user = userEvent.setup();
      await user.type(await screen.findByRole('textbox', { name: 'Tag' }), tag);
      expect(
        screen.getByText(
          'This tag already has a color. Change it in its row above.',
        ),
      ).toBeTruthy();
      expect(
        (screen.getByRole('button', { name: 'Add color' }) as HTMLButtonElement)
          .disabled,
      ).toBe(true);
      await user.keyboard('{Enter}');
      expect(updateTagColors).not.toHaveBeenCalled();
    },
  );

  it('adds a color for a new tag', async () => {
    vi.mocked(getTagColors).mockResolvedValue(colors);
    vi.mocked(updateTagColors).mockResolvedValue(colors);
    renderSection();
    const user = userEvent.setup();
    await user.type(
      await screen.findByRole('textbox', { name: 'Tag' }),
      'fresh',
    );
    await user.click(screen.getByRole('button', { name: 'Add color' }));
    expect(vi.mocked(updateTagColors).mock.calls[0]?.[0]).toMatchObject({
      set: { fresh: expect.any(String) },
    });
  });
});
