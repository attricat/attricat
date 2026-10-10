// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../../i18n';
import { EmailEditor, PhoneEditor, UrlEditor } from './editors';

const attribute = (code: string) => ({
  code,
  name: code,
  value_type: 'string' as const,
});
const editorProps = (code: string, value: string) => ({
  attribute: attribute(code),
  disabled: false,
  onChange: () => {},
  value,
});

describe('link editors', () => {
  it('opens a valid URL in a new tab from inside the field', () => {
    render(<UrlEditor {...editorProps('website', 'https://example.com/a')} />);
    const link = screen.getByRole('link', {
      name: 'https://example.com/a (opens in a new tab)',
    });
    expect(link.getAttribute('href')).toBe('https://example.com/a');
    expect(link.getAttribute('target')).toBe('_blank');
    expect(link.getAttribute('rel')).toBe('noopener noreferrer');
  });

  it('offers mail and call actions for valid addresses and numbers', () => {
    render(
      <>
        <EmailEditor {...editorProps('email', 'ada@example.com')} />
        <PhoneEditor {...editorProps('phone', '+1 202 555 0123')} />
      </>,
    );
    expect(
      screen
        .getByRole('link', { name: 'Email ada@example.com' })
        .getAttribute('href'),
    ).toBe('mailto:ada@example.com');
    expect(
      screen
        .getByRole('link', { name: 'Call +1 202 555 0123' })
        .getAttribute('href'),
    ).toBe('tel:+12025550123');
  });

  it('offers no action for empty or invalid values', () => {
    render(
      <>
        <UrlEditor {...editorProps('website', 'javascript:alert(1)')} />
        <EmailEditor {...editorProps('email', '')} />
        <PhoneEditor {...editorProps('phone', '555 0123')} />
      </>,
    );
    expect(screen.queryByRole('link')).toBeNull();
  });
});
