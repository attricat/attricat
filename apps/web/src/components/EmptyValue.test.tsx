// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import '../i18n';
import { EmptyValue } from './EmptyValue';

describe('EmptyValue', () => {
  it('shows a dash that assistive technology reads as not set', () => {
    const { container } = render(<EmptyValue />);
    expect(container.querySelector('[aria-hidden="true"]')?.textContent).toBe(
      '—',
    );
    expect(screen.getByText('Not set')).toBeTruthy();
  });

  it('names the dash on hover', async () => {
    const user = userEvent.setup();
    render(<EmptyValue />);
    await user.hover(screen.getByText('—'));
    expect((await screen.findByRole('tooltip')).textContent).toBe('Not set');
  });

  it('says what is missing when given a label', async () => {
    const user = userEvent.setup();
    render(<EmptyValue label="No files added" />);
    expect(screen.queryByText('Not set')).toBeNull();
    await user.hover(screen.getByText('—'));
    expect((await screen.findByRole('tooltip')).textContent).toBe(
      'No files added',
    );
    // Screen readers get the same, more specific label.
    expect(screen.getAllByText('No files added')).toHaveLength(2);
  });
});
