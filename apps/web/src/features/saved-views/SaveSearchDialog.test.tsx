// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { SaveSearchDialog } from './SaveSearchDialog';

describe('SaveSearchDialog', () => {
  it('asks for a name instead of ignoring one made of spaces', async () => {
    const onSave = vi.fn();
    const user = userEvent.setup();
    render(
      <SaveSearchDialog
        error={null}
        isPending={false}
        onClose={() => undefined}
        onSave={onSave}
      />,
    );
    const name = screen.getByRole('textbox', { name: /^Name/ });
    await user.type(name, '   ');
    await user.click(screen.getByRole('button', { name: 'Save search' }));
    expect(await screen.findByText('This field is required.')).toBeTruthy();
    expect(onSave).not.toHaveBeenCalled();

    await user.clear(name);
    await user.type(name, '  Low stock  ');
    await user.click(screen.getByRole('button', { name: 'Save search' }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'Low stock' }),
    );
  });
});
