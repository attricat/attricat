// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { definitionKinds } from './constants';
import { DefinitionInsertMenu } from './DefinitionInsertMenu';

const definition = `code: size
name: Size
value_type: string
value_schema:
  type: string
  enum: [s]
`;

const renderMenu = () => {
  const onInsert = vi.fn();
  render(
    <DefinitionInsertMenu
      definition={definition}
      disabled={false}
      kind={definitionKinds.reusableAttribute}
      onInsert={onInsert}
    />,
  );
  fireEvent.click(screen.getByRole('button', { name: 'Insert' }));
  return onInsert;
};

describe('DefinitionInsertMenu', () => {
  it('lists kinds of snippet with how many can still be added', () => {
    renderMenu();
    expect(
      screen.getAllByRole('menuitem').map((item) => item.textContent),
    ).toEqual([
      'Values1 of 6 available',
      'Behavior4 of 4 available',
      'Relationships and files2 of 2 available',
    ]);
  });

  it('opens a kind, describes its snippets, and inserts one', async () => {
    const onInsert = renderMenu();
    fireEvent.click(screen.getByRole('menuitem', { name: /^Values/ }));
    const back = screen.getByRole('menuitem', { name: 'Back' });
    await waitFor(() => expect(document.activeElement).toBe(back));
    expect(
      screen
        .getByRole('menuitem', { name: /Allowed values/ })
        .getAttribute('aria-disabled'),
    ).toBe('true');
    expect(screen.getAllByText('Already defined')).toHaveLength(5);
    fireEvent.click(
      screen.getByRole('menuitem', {
        name: 'Default value The value new records start with.',
      }),
    );
    expect(onInsert).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'defaultValue' }),
    );
  });

  it('goes back to the kinds', () => {
    renderMenu();
    fireEvent.click(screen.getByRole('menuitem', { name: /^Behavior/ }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Back' }));
    expect(screen.getByRole('menuitem', { name: /^Values/ })).toBeTruthy();
  });
});
