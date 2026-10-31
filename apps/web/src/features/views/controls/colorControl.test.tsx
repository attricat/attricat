// @vitest-environment jsdom
import { useState } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ThemeProvider } from '@mui/material';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { makeTheme } from '../../../app/theme';
import { ColorEditor } from './editors';
import { ColorValue } from './values';
import { RecordView } from '../components/RecordView';
import { BlueprintViewsPreview } from '../../blueprints/BlueprintViewsPreview';
import { ConfiguredColumnCell } from '../../explorer/ConfiguredColumnCell';
import type { RecordItem } from '../../records/api';

const attribute = { code: 'hex', value_type: 'string' as const };
const display = { id: 'attricat.color_display', version: 1, props: {} };

describe('color components', () => {
  it.each(['light', 'dark'] as const)(
    'renders labeled swatches and safe fallbacks in %s mode',
    (mode) => {
      const { container } = render(
        <ThemeProvider theme={makeTheme(mode)}>
          <ColorValue
            attribute={attribute}
            value={['#ffffff', '#000000', 'url(evil)', '<script>evil</script>']}
          />
        </ThemeProvider>,
      );
      expect(container.querySelectorAll('[aria-hidden="true"]')).toHaveLength(
        2,
      );
      expect(screen.getByText('#ffffff')).toBeTruthy();
      expect(screen.getByText('#000000')).toBeTruthy();
      expect(screen.getByText('url(evil)')).toBeTruthy();
      expect(container.querySelector('script')).toBeNull();
    },
  );
  it.each([null, undefined, '', []])(
    'shows missing %j without a swatch',
    (value) => {
      const { container } = render(
        <ColorValue attribute={attribute} value={value} />,
      );
      expect(screen.getByText('Not set')).toBeTruthy();
      expect(container.querySelector('[aria-hidden="true"]')).toBeNull();
    },
  );
  it('keeps partial text and synchronizes the picker without saving a default', () => {
    const Editor = () => {
      const [value, setValue] = useState('');
      return (
        <ColorEditor
          attribute={attribute}
          value={value}
          onChange={setValue}
          disabled={false}
        />
      );
    };
    render(<Editor />);
    const picker = screen.getByLabelText('Pick color for hex');
    fireEvent.click(picker);
    // Only the picker shows; the hex text waits in a popover.
    expect(screen.queryByRole('textbox')).toBeNull();
    expect(screen.getByText('Not set')).toBeTruthy();
    fireEvent.click(
      screen.getByRole('button', { name: 'Edit hex value of hex' }),
    );
    const text = screen.getByRole('textbox', { name: 'hex' });
    expect(text).toHaveProperty('value', '');
    fireEvent.change(text, { target: { value: '#1' } });
    expect(text).toHaveProperty('value', '#1');
    expect(text.getAttribute('aria-invalid')).toBe('true');
    fireEvent.change(picker, { target: { value: '#123456' } });
    expect(text).toHaveProperty('value', '#123456');
    fireEvent.change(text, { target: { value: '#aBcDeF' } });
    expect(picker).toHaveProperty('value', '#abcdef');
    fireEvent.change(text, { target: { value: '' } });
    expect(text).toHaveProperty('value', '');
  });
  it('blocks every input path when disabled', () => {
    const onChange = vi.fn();
    render(
      <ColorEditor
        attribute={attribute}
        value="#ffffff"
        onChange={onChange}
        disabled
      />,
    );
    expect(
      screen.getByRole('button', { name: 'Edit hex value of hex' }),
    ).toHaveProperty('disabled', true);
    const picker = screen.getByLabelText('Pick color for hex');
    expect(picker).toHaveProperty('disabled', true);
    fireEvent.change(picker, { target: { value: '#000000' } });
    expect(onChange).not.toHaveBeenCalled();
  });
  it('copies the value on click, even when read-only', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
    render(
      <ColorEditor
        attribute={attribute}
        value="#aBcDeF"
        onChange={vi.fn()}
        disabled
      />,
    );
    const value = screen.getByRole('button', { name: 'Copy #aBcDeF' });
    expect(value).toHaveProperty('disabled', false);
    fireEvent.click(value);
    await waitFor(() => expect(writeText).toHaveBeenCalledWith('#aBcDeF'));
  });
  it('uses explicit field configuration only', () => {
    const { container } = render(
      <RecordView
        attributes={[attribute]}
        values={{ hex: { value: '#123456' } }}
        view={{
          type: 'stack',
          children: [{ type: 'field', field: 'hex', component: display }],
        }}
      />,
    );
    expect(container.querySelector('[aria-hidden="true"]')).toBeTruthy();
    expect(screen.queryByRole('textbox')).toBeNull();
  });
  it('previews editing and configured table columns with the same value', () => {
    const { container } = render(
      // The sandbox's field diagnostics read the session.
      <QueryClientProvider client={new QueryClient()}>
        <BlueprintViewsPreview
          attributes={[attribute]}
          views={{
            detail: {
              type: 'stack',
              children: [{ type: 'field', field: 'hex', component: display }],
            },
            table: {
              type: 'table',
              columns: [{ field: 'hex', label: 'Color', renderer: display }],
            },
          }}
        />
      </QueryClientProvider>,
    );
    fireEvent.change(screen.getByLabelText('Pick color for hex'), {
      target: { value: '#123456' },
    });
    fireEvent.click(screen.getByRole('tab', { name: 'table' }));
    expect(screen.getByText('Color')).toBeTruthy();
    expect(screen.getByText('#123456')).toBeTruthy();
    expect(container.querySelector('[aria-hidden="true"]')).toBeTruthy();
  });
  it('renders projected colors in explorer without extension execution', () => {
    const { container } = render(
      <ConfiguredColumnCell
        attribute={attribute}
        column={{
          field: 'color.hex',
          renderer: display,
          sortable: false,
          relationshipSortBlocked: false,
        }}
        record={
          {
            table_values: { 'color.hex': ['#ffffff', '#000000'] },
          } as unknown as RecordItem
        }
        extension={undefined}
      />,
    );
    expect(container.querySelectorAll('[aria-hidden="true"]')).toHaveLength(2);
    expect(container.querySelector('iframe')).toBeNull();
  });
});
