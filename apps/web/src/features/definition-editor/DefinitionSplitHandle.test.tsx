// @vitest-environment jsdom
import { act, render, renderHook, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import '../../i18n';
import { useDefinitionLayout } from './definitionLayout';
import { DefinitionSplitHandle } from './DefinitionSplitHandle';

const Workspace = () => {
  const layout = useDefinitionLayout();
  return (
    <DefinitionSplitHandle
      editorShare={layout.editorShare}
      onCommit={layout.commitResize}
      onResize={layout.resize}
    />
  );
};

afterEach(() => localStorage.clear());

describe('DefinitionSplitHandle', () => {
  it('resizes with the arrow keys within its limits', async () => {
    const user = userEvent.setup();
    render(<Workspace />);
    const handle = screen.getByRole('separator', {
      name: 'Resize the editor and preview',
    });
    expect(handle.getAttribute('aria-valuenow')).toBe('60');

    handle.focus();
    await user.keyboard('{ArrowLeft}');
    expect(handle.getAttribute('aria-valuenow')).toBe('55');
    await user.keyboard('{End}{ArrowRight}');
    expect(handle.getAttribute('aria-valuenow')).toBe('80');
  });
});

describe('useDefinitionLayout', () => {
  it('remembers the kept width and hidden preview', () => {
    const { result } = renderHook(useDefinitionLayout);
    act(() => result.current.togglePreview());
    act(() => result.current.commitResize(0.1));
    expect(renderHook(useDefinitionLayout).result.current).toMatchObject({
      editorShare: 0.3,
      previewHidden: true,
    });
  });
});
