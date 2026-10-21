// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../i18n';
import { JsonBlock, JsonInline } from './JsonValue';

describe('JsonInline', () => {
  it('renders a value as one line of code', () => {
    const { container } = render(<JsonInline value={{ a: [1, 2] }} />);
    expect(container.querySelector('code')?.textContent).toBe(
      '{ "a": [1, 2] }',
    );
  });
});

describe('JsonBlock', () => {
  it('collapses nested members beyond the default depth', () => {
    render(
      <JsonBlock value={{ outer: { inner: { deep: 1 } }, tags: ['a', 'b'] }} />,
    );
    expect(
      screen
        .getByRole('button', { name: 'Collapse outer' })
        .getAttribute('aria-expanded'),
    ).toBe('true');
    const inner = screen.getByRole('button', { name: 'Expand inner' });
    expect(inner.getAttribute('aria-expanded')).toBe('false');
    expect(screen.getByText('"deep"')).toBeTruthy();

    fireEvent.click(inner);
    expect(screen.getByRole('button', { name: 'Collapse inner' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Copy JSON' })).toBeTruthy();
  });

  it('shows large arrays in pages', () => {
    render(
      <JsonBlock value={Array.from({ length: 60 }, (_, index) => index)} />,
    );
    expect(screen.queryByText('59')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Show all 60' }));
    expect(screen.getByText('59')).toBeTruthy();
  });

  it('keeps a scalar inline', () => {
    const { container } = render(<JsonBlock value="text" />);
    expect(container.textContent).toBe('"text"');
    expect(screen.queryByRole('button')).toBeNull();
  });
});
