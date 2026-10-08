// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { EmptyState } from './EmptyState';
import { TeamIcon } from './systemIcons';

describe('EmptyState', () => {
  it('shows the title and description with a decorative icon', () => {
    const { container } = render(
      <EmptyState
        description="Teams group people for assignments."
        icon={TeamIcon}
        title="No teams yet."
      />,
    );
    expect(screen.getByText('No teams yet.').tagName).toBe('P');
    expect(
      screen.getByText('Teams group people for assignments.'),
    ).toBeTruthy();
    expect(container.querySelector('svg')?.getAttribute('aria-hidden')).toBe(
      'true',
    );
  });

  it('can render its title as a section heading', () => {
    render(
      <EmptyState icon={TeamIcon} title="No teams yet." titleComponent="h3" />,
    );
    expect(
      screen.getByRole('heading', { level: 3, name: 'No teams yet.' }),
    ).toBeTruthy();
  });
});
