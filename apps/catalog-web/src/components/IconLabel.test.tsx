// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { IconLabel } from './IconLabel';
import { compactIconSize } from './iconSizes';
import { TeamIcon } from './systemIcons';

describe('IconLabel', () => {
  it('leads the label with a decorative compact icon', () => {
    const { container } = render(<IconLabel icon={TeamIcon}>Team</IconLabel>);
    const icon = container.querySelector('svg');

    expect(screen.getByText('Team')).toBeTruthy();
    expect(icon?.getAttribute('aria-hidden')).toBe('true');
    expect(icon?.getAttribute('width')).toBe(String(compactIconSize));
    expect(container.textContent).toBe('Team');
  });
});
