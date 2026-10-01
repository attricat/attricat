// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@mui/material';
import { makeTheme } from '../../../app/theme';
import '../../../i18n';
import { UrlDisplay } from './UrlDisplay';
import { UrlEditor } from './UrlEditor';

describe('URL components', () => {
  it.each(['light', 'dark'] as const)(
    'renders safe accessible links in %s mode without row activation',
    (mode) => {
      const rowClick = vi.fn();
      render(
        <ThemeProvider theme={makeTheme(mode)}>
          <div
            role="button"
            tabIndex={0}
            onClick={rowClick}
            onKeyDown={rowClick}
          >
            <UrlDisplay value="https://example.com/path" />
          </div>
        </ThemeProvider>,
      );
      const link = screen.getByRole('link', {
        name: /https:\/\/example.com\/path/,
      });
      expect(link.getAttribute('target')).toBe('_blank');
      expect(link.getAttribute('rel')).toBe('noopener noreferrer');
      fireEvent.click(link);
      expect(rowClick).not.toHaveBeenCalled();
    },
  );
  it('keeps unsafe, empty, and multiple projected values readable', () => {
    render(
      <UrlDisplay
        value={['javascript:alert(1)', null, 'https://example.com']}
      />,
    );
    expect(screen.getAllByRole('link')).toHaveLength(1);
    expect(screen.getByText('javascript:alert(1)')).toBeTruthy();
  });
  it('preserves input and exposes validation and disabled state', () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <UrlEditor label="Website" value="bad URL" onChange={onChange} />,
    );
    const input = screen.getByRole('textbox', { name: 'Website' });
    expect(input.getAttribute('aria-invalid')).toBe('true');
    fireEvent.change(input, { target: { value: 'https://example.com/a?b=2' } });
    expect(onChange).toHaveBeenCalledWith('https://example.com/a?b=2');
    onChange.mockClear();
    rerender(
      <UrlEditor
        label="Website"
        value="https://example.com"
        onChange={onChange}
        disabled
      />,
    );
    expect((input as HTMLInputElement).disabled).toBe(true);
    fireEvent.change(input, { target: { value: 'https://other.example.com' } });
    expect(onChange).not.toHaveBeenCalled();
  });
});
