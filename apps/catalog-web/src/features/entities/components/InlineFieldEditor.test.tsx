// @vitest-environment jsdom
import { MenuItem, TextField } from '@mui/material';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { InlineFieldEditor } from './InlineFieldEditor';

const renderText = (
  options: { validate?: (value: string) => string | undefined } = {},
) => {
  const onCommit = vi.fn();
  const onRevert = vi.fn();
  render(
    <>
      <InlineFieldEditor
        onCommit={onCommit}
        onRevert={onRevert}
        validate={options.validate}
        value="Saved"
      >
        {({ value, error, onChange }) => (
          <TextField
            error={Boolean(error)}
            helperText={error}
            label="Title"
            onChange={(event) => onChange(event.target.value)}
            value={value}
          />
        )}
      </InlineFieldEditor>
      <button type="button">Elsewhere</button>
    </>,
  );
  return { onCommit, onRevert };
};

describe('InlineFieldEditor', () => {
  it('commits typed text when focus leaves the field', async () => {
    const user = userEvent.setup();
    const { onCommit } = renderText();
    const input = screen.getByLabelText('Title');

    await user.clear(input);
    await user.type(input, 'New');
    expect(onCommit).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Elsewhere' }));
    expect(onCommit).toHaveBeenCalledExactlyOnceWith('New');
  });

  it('commits a single-line input on Enter', async () => {
    const user = userEvent.setup();
    const { onCommit } = renderText();

    await user.type(screen.getByLabelText('Title'), '!{Enter}');
    expect(onCommit).toHaveBeenCalledExactlyOnceWith('Saved!');
  });

  it('returns to the saved value on Escape', async () => {
    const user = userEvent.setup();
    const { onCommit, onRevert } = renderText();
    const input = screen.getByLabelText('Title');

    await user.type(input, ' draft{Escape}');
    expect((input as HTMLInputElement).value).toBe('Saved');
    expect(onRevert).toHaveBeenCalledOnce();

    await user.click(screen.getByRole('button', { name: 'Elsewhere' }));
    expect(onCommit).not.toHaveBeenCalled();
  });

  it('keeps an invalid value local and shows why', async () => {
    const user = userEvent.setup();
    const { onCommit } = renderText({
      validate: (value) =>
        value.includes('!') ? 'No exclamations' : undefined,
    });

    await user.type(screen.getByLabelText('Title'), '!');
    await user.click(screen.getByRole('button', { name: 'Elsewhere' }));

    expect(onCommit).not.toHaveBeenCalled();
    expect(screen.getByText('No exclamations')).toBeTruthy();
    expect((screen.getByLabelText('Title') as HTMLInputElement).value).toBe(
      'Saved!',
    );
  });

  it('commits a choice made without typing at once', async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    render(
      <InlineFieldEditor onCommit={onCommit} onRevert={vi.fn()} value="a">
        {({ value, onChange }) => (
          <TextField
            label="Choice"
            onChange={(event) => onChange(event.target.value)}
            select
            value={value}
          >
            <MenuItem value="a">A</MenuItem>
            <MenuItem value="b">B</MenuItem>
          </TextField>
        )}
      </InlineFieldEditor>,
    );

    await user.click(screen.getByRole('combobox', { name: 'Choice' }));
    await user.click(screen.getByRole('option', { name: 'B' }));
    expect(onCommit).toHaveBeenCalledExactlyOnceWith('b');
  });
});
