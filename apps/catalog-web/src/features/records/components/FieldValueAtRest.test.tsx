// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { FieldValueAtRest } from './FieldValueAtRest';

describe('FieldValueAtRest', () => {
  it('shows the value under its label and opens the editor on request', async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn();
    render(
      <FieldValueAtRest
        attribute={{ code: 'care_notes', value_type: 'string' }}
        onEdit={onEdit}
      >
        <p>Wipe dry</p>
      </FieldValueAtRest>,
    );

    expect(screen.getByText('care notes').textContent).toBe('care notes');
    expect(screen.getByText('Wipe dry')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Edit care notes' }));
    expect(onEdit).toHaveBeenCalledOnce();
  });

  it('marks a required field visually without changing its name', () => {
    render(
      <FieldValueAtRest
        attribute={{ code: 'notes', name: 'Notes', value_type: 'string' }}
        onEdit={() => {}}
        required
      >
        <p>Wipe dry</p>
      </FieldValueAtRest>,
    );

    expect(screen.getByText('Notes').textContent).toBe('Notes *');
    expect(screen.getByRole('button', { name: 'Edit Notes' })).toBeTruthy();
  });
});
