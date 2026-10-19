// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../../i18n';
import type { Attribute } from '../../records/api';
import { ScalarAttributeEditor } from '../../records/components/ScalarAttributeEditor';
import {
  attributeFilterValueLabel,
  operatorsForAttribute,
} from '../../explorer/attributeFilters';
import { formatAttributeValue } from '../components/values/formatAttributeValue';

const size = {
  code: 'size',
  name: 'Size',
  value_type: 'string',
  value_schema: {
    enum: ['s', 'l'],
    type: 'string',
    'x-attricat-enum-labels': { s: 'Small' },
  },
} as Attribute;

afterEach(cleanup);

describe('enum choices', () => {
  it('edits a short list with labelled radio buttons', () => {
    const onChange = vi.fn();
    render(
      <ScalarAttributeEditor
        attribute={size}
        disabled={false}
        onChange={onChange}
        value=""
      />,
    );
    const group = screen.getByRole('radiogroup', { name: 'Size' });
    expect(
      within(group)
        .getAllByRole('radio')
        .map((radio) => radio.closest('label')?.textContent),
    ).toEqual(['Not set', 'Small', 'l']);
    fireEvent.click(screen.getByRole('radio', { name: 'Small' }));
    expect(onChange).toHaveBeenCalledWith('s');
  });

  it('offers no way back to unset when a value is required', () => {
    render(
      <ScalarAttributeEditor
        attribute={size}
        disabled={false}
        onChange={vi.fn()}
        required
        value="s"
      />,
    );
    expect(screen.queryByRole('radio', { name: 'Not set' })).toBeNull();
  });

  it('searches a long list of choices', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const country = {
      code: 'country',
      name: 'Country',
      value_type: 'string',
      value_schema: {
        type: 'string',
        enum: ['pl', 'de', 'fr', 'es', 'it', 'nl'],
        'x-attricat-enum-labels': { pl: 'Poland', nl: 'Netherlands' },
      },
    } as Attribute;
    render(
      <ScalarAttributeEditor
        attribute={country}
        disabled={false}
        onChange={onChange}
        value="pl"
      />,
    );
    const input = screen.getByRole('combobox', { name: 'Country' });
    expect(input).toHaveProperty('value', 'Poland');
    await user.clear(input);
    expect(onChange).not.toHaveBeenCalled();
    await user.type(input, 'nether');
    await user.click(screen.getByRole('option', { name: 'Netherlands' }));
    expect(onChange).toHaveBeenCalledWith('nl');
  });

  it('keeps a value that is no longer allowed visible', () => {
    render(
      <ScalarAttributeEditor
        attribute={size}
        disabled={false}
        onChange={vi.fn()}
        value="xl"
      />,
    );
    expect(screen.getByText('Not an allowed value: xl')).toBeTruthy();
  });

  it('shows labels and filters by exact value', () => {
    expect(formatAttributeValue(size, 's')).toBe('Small');
    expect(formatAttributeValue(size, 'l')).toBe('l');
    expect(operatorsForAttribute(size)).toEqual(['eq', 'is_set']);
    expect(
      attributeFilterValueLabel(
        i18n.t,
        { field: 'size', operator: 'eq', value: 's' },
        size,
      ),
    ).toBe('Small');
  });
});
