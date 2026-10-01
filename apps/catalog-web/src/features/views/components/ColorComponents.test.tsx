// @vitest-environment jsdom
import { useState } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { ThemeProvider } from '@mui/material';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { makeTheme } from '../../../app/theme';
import { ColorAttributeEditor } from '../../entities/components/ColorAttributeEditor';
import { ColorValue } from './values/ColorValue';
import { EntityView } from './EntityView';
import { BlueprintViewsPreview } from '../../blueprints/BlueprintViewsPreview';
import { ConfiguredColumnCell } from '../../explorer/ConfiguredColumnCell';
import type { EntityItem } from '../../entities/api';

const attribute = { code: 'hex', value_type: 'string' as const };
const display = { id: 'catalog.color_display', version: 1, props: {} };
const edit = { id: 'catalog.color_edit', version: 1, props: {} };

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
        <ColorAttributeEditor
          attribute={attribute}
          value={value}
          onChange={setValue}
          disabled={false}
        />
      );
    };
    render(<Editor />);
    const text = screen.getByRole('textbox', { name: 'hex' });
    const picker = screen.getByLabelText('Pick color for hex');
    fireEvent.click(picker);
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
      <ColorAttributeEditor
        attribute={attribute}
        value="#ffffff"
        onChange={onChange}
        disabled
      />,
    );
    for (const input of [
      screen.getByRole('textbox'),
      screen.getByLabelText('Pick color for hex'),
    ]) {
      expect(input).toHaveProperty('disabled', true);
      fireEvent.change(input, { target: { value: '#000000' } });
    }
    expect(onChange).not.toHaveBeenCalled();
  });
  it('uses explicit field configuration only', () => {
    const { container } = render(
      <EntityView
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
      <BlueprintViewsPreview
        attributes={[attribute]}
        views={{
          edit: {
            type: 'stack',
            children: [{ type: 'field', field: 'hex', component: edit }],
          },
          table: {
            type: 'table',
            fields: [],
            columns: [{ field: 'hex', label: 'Color', renderer: display }],
          },
        }}
      />,
    );
    fireEvent.change(screen.getByRole('textbox', { name: 'hex' }), {
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
        entity={
          {
            table_values: { 'color.hex': ['#ffffff', '#000000'] },
          } as unknown as EntityItem
        }
        extension={undefined}
        frameAllowed={false}
      />,
    );
    expect(container.querySelectorAll('[aria-hidden="true"]')).toHaveLength(2);
    expect(container.querySelector('iframe')).toBeNull();
  });
});
