// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../i18n';
import { useToastStore } from './toastStore';
import { useCopyToClipboard } from './useCopyToClipboard';

const copy = async (value: string) => {
  const { result } = renderHook(() => useCopyToClipboard());
  await act(() => result.current(value));
};

const lastToast = () => useToastStore.getState().toasts.at(-1);

const stubClipboard = (clipboard: Partial<Clipboard> | undefined) =>
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: clipboard,
  });

const stubExecCommand = (implementation: (command: string) => boolean) => {
  const execCommand = vi.fn(implementation);
  Object.defineProperty(document, 'execCommand', {
    configurable: true,
    value: execCommand,
  });
  return execCommand;
};

describe('useCopyToClipboard', () => {
  beforeEach(() => useToastStore.setState({ toasts: [] }));
  afterEach(() => {
    Reflect.deleteProperty(navigator, 'clipboard');
    Reflect.deleteProperty(document, 'execCommand');
  });

  it('copies through the Clipboard API when the page has it', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    stubClipboard({ writeText });
    const execCommand = stubExecCommand(() => true);

    await copy('record-1');

    expect(writeText).toHaveBeenCalledWith('record-1');
    expect(execCommand).not.toHaveBeenCalled();
    expect(lastToast()).toMatchObject({
      message: 'Copied to clipboard.',
      severity: 'success',
    });
  });

  it('falls back to a hidden text area outside secure contexts', async () => {
    stubClipboard(undefined);
    const button = document.createElement('button');
    document.body.append(button);
    button.focus();
    let copied: string | undefined;
    const execCommand = stubExecCommand(() => {
      const selection = document.activeElement;
      copied =
        selection instanceof HTMLTextAreaElement ? selection.value : undefined;
      return true;
    });

    await copy('record-1');

    expect(execCommand).toHaveBeenCalledWith('copy');
    expect(copied).toBe('record-1');
    expect(document.querySelector('textarea')).toBeNull();
    expect(document.activeElement).toBe(button);
    expect(lastToast()).toMatchObject({
      message: 'Copied to clipboard.',
      severity: 'success',
    });
    button.remove();
  });

  it('reports a failure when the fallback cannot copy either', async () => {
    stubClipboard(undefined);
    stubExecCommand(() => false);

    await copy('record-1');

    expect(document.querySelector('textarea')).toBeNull();
    expect(lastToast()).toMatchObject({
      message: "Couldn't copy to clipboard.",
      severity: 'error',
    });
  });

  it('keeps the text area inside a dialog so its focus trap allows the copy', async () => {
    stubClipboard(undefined);
    const dialog = document.createElement('div');
    dialog.setAttribute('role', 'dialog');
    const button = document.createElement('button');
    dialog.append(button);
    document.body.append(dialog);
    button.focus();
    let insideDialog = false;
    stubExecCommand(() => {
      insideDialog = dialog.contains(document.activeElement);
      return true;
    });

    await copy('token');

    expect(insideDialog).toBe(true);
    dialog.remove();
  });
});
