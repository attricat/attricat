// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
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
  it('edits with a select of labelled values', () => {
    const onChange = vi.fn();
    render(
      <ScalarAttributeEditor
        attribute={size}
        disabled={false}
        onChange={onChange}
        value=""
      />,
    );
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Size' }));
    expect(
      screen.getAllByRole('option').map((option) => option.textContent),
    ).toEqual(['Not set', 'Small', 'l']);
    fireEvent.click(screen.getByRole('option', { name: 'Small' }));
    expect(onChange).toHaveBeenCalledWith('s');
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
