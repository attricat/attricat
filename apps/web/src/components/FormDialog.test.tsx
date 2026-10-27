// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../i18n';
import { FormDialog } from './FormDialog';

const renderDialog = (isPending = false) => {
  const onClose = vi.fn();
  render(
    <FormDialog
      isPending={isPending}
      onClose={onClose}
      onSubmit={vi.fn()}
      submitLabel="Save"
      title="Rename"
    >
      <input aria-label="Name" />
    </FormDialog>,
  );
  return onClose;
};

const backdrop = () => {
  const element = document.querySelector('.MuiBackdrop-root');
  if (!element) throw new Error('The dialog has no backdrop');
  return element;
};

describe('FormDialog', () => {
  it('keeps typed input when the backdrop is clicked', () => {
    const onClose = renderDialog();
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'Draft' },
    });
    fireEvent.mouseDown(backdrop());
    fireEvent.click(backdrop());
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByLabelText<HTMLInputElement>('Name').value).toBe('Draft');
  });

  it('closes on Escape and Cancel', () => {
    const onClose = renderDialog();
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('does not close on Escape while it saves', () => {
    const onClose = renderDialog(true);
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).not.toHaveBeenCalled();
  });
});
