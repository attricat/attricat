// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@mui/material';
import { makeTheme } from '../../../app/theme';
import '../../../i18n';
import { EmailEditor } from './editors';
import { EmailValue } from './values';
import { RecordView } from '../components/RecordView';
import { BlueprintViewsPreview } from '../../blueprints/BlueprintViewsPreview';

const emailDisplay = { id: 'attricat.email_display', version: 1, props: {} };
const emailEdit = { id: 'attricat.email_edit', version: 1, props: {} };
const attribute = { code: 'contact', value_type: 'string' as const };

describe('email components', () => {
  it.each(['light', 'dark'] as const)(
    'renders display links and empty values in %s mode',
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
            <EmailValue
              attribute={attribute}
              value={['Name+tag@Example.com', 'not an email', null]}
            />
          </div>
        </ThemeProvider>,
      );
      const link = screen.getByRole('link', { name: 'Name+tag@Example.com' });
      expect(link.getAttribute('href')).toBe('mailto:Name%2Btag@Example.com');
      link.addEventListener('click', (event) => event.preventDefault(), {
        once: true,
      });
      fireEvent.keyDown(link, { key: 'Enter' });
      fireEvent.click(link);
      expect(rowClick).not.toHaveBeenCalled();
      expect(screen.getByText('not an email').closest('a')).toBeNull();
      expect(screen.getByText('Not set')).toBeTruthy();
    },
  );
  it('renders a configured field without an editor', () => {
    render(
      <RecordView
        attributes={[attribute]}
        values={{ contact: { value: 'a@example.test' } }}
        view={{
          type: 'stack',
          children: [
            { type: 'field', field: 'contact', component: emailDisplay },
          ],
        }}
      />,
    );
    expect(screen.getByRole('link')).toBeTruthy();
    expect(screen.queryByRole('textbox')).toBeNull();
  });
  it('labels the email input and connects actionable errors', () => {
    render(
      <EmailEditor
        attribute={attribute}
        value="invalid"
        onChange={vi.fn()}
        disabled={false}
        required
      />,
    );
    const input = screen.getByRole('textbox', { name: /contact/ });
    expect(input.getAttribute('type')).toBe('email');
    expect(input.getAttribute('aria-invalid')).toBe('true');
    expect(
      document.getElementById(input.getAttribute('aria-describedby')!)
        ?.textContent,
    ).toMatch(/single email address/);
    expect(input).toHaveProperty('required', true);
  });
  it('blocks even dispatched change events while disabled', () => {
    const onChange = vi.fn();
    render(
      <EmailEditor
        attribute={attribute}
        value="a@example.test"
        onChange={onChange}
        disabled
      />,
    );
    fireEvent.change(screen.getByRole('textbox'), {
      target: { value: 'b@example.test' },
    });
    expect(onChange).not.toHaveBeenCalled();
  });
  it('uses email input and display in the blueprint sandbox', () => {
    render(
      <BlueprintViewsPreview
        attributes={[attribute]}
        views={{
          edit: {
            type: 'stack',
            children: [
              { type: 'field', field: 'contact', component: emailEdit },
            ],
          },
          detail: {
            type: 'stack',
            children: [
              { type: 'field', field: 'contact', component: emailDisplay },
            ],
          },
          table: {
            type: 'table',
            fields: [],
            columns: [{ field: 'contact', renderer: emailDisplay }],
          },
        }}
      />,
    );
    expect(screen.getByRole('textbox').getAttribute('type')).toBe('email');
    fireEvent.change(screen.getByRole('textbox'), {
      target: { value: 'a@example.test' },
    });
    fireEvent.click(screen.getByRole('tab', { name: 'detail' }));
    expect(screen.getByRole('link', { name: 'a@example.test' })).toBeTruthy();
    fireEvent.click(screen.getByRole('tab', { name: 'table' }));
    expect(screen.getByRole('link', { name: 'a@example.test' })).toBeTruthy();
  });
});
