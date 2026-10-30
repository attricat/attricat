// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { editor } from 'monaco-editor';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { DefinitionFoldControls } from './DefinitionFoldControls';

describe('DefinitionFoldControls', () => {
  it("runs Monaco's fold and unfold actions", async () => {
    const run = vi.fn(() => Promise.resolve());
    const getAction = vi.fn(() => ({ run }));
    const user = userEvent.setup();

    render(
      <DefinitionFoldControls
        editor={{ getAction } as unknown as editor.IStandaloneCodeEditor}
      />,
    );
    await user.click(screen.getByRole('button', { name: 'Fold all' }));
    await user.click(screen.getByRole('button', { name: 'Unfold all' }));

    expect(getAction.mock.calls).toEqual([
      ['editor.foldAll'],
      ['editor.unfoldAll'],
    ]);
    expect(run).toHaveBeenCalledTimes(2);
  });

  it('is disabled until the editor mounts', () => {
    render(<DefinitionFoldControls />);

    expect(screen.getByRole('button', { name: 'Fold all' })).toHaveProperty(
      'disabled',
      true,
    );
    expect(screen.getByRole('button', { name: 'Unfold all' })).toHaveProperty(
      'disabled',
      true,
    );
  });
});
