import { describe, expect, it } from 'vitest';
import {
  attributeLabel,
  contextDisplayLabel,
  displayLabel,
  dropdownOptionLabel,
} from './entityDisplay';

describe('displayLabel', () => {
  it('uses an attribute name and humanizes its code as a fallback', () => {
    expect(attributeLabel({ code: 'product_family', name: 'Family' })).toBe(
      'Family',
    );
    expect(attributeLabel({ code: 'product_family' })).toBe('product family');
  });

  it('uses the backend default label and falls back to the entity ID', () => {
    expect(displayLabel({ default: 'Summer shirt' }, 'entity-id')).toBe(
      'Summer shirt',
    );
    expect(displayLabel({}, 'entity-id')).toBe('entity-id');
  });

  it('renders a dropdown option from its configured target fields', () => {
    expect(
      dropdownOptionLabel(
        { default: { name: 'Navy', hex: '#1c2d4a' } },
        {
          dropdown_option: {
            type: 'dropdown_option',
            fields: ['name', 'hex'],
            separator: ' / ',
          },
        },
      ),
    ).toBe('Navy / #1c2d4a');
  });
});

describe('contextDisplayLabel', () => {
  it('uses the nearest context on the path that has a label', () => {
    const display = { default: 'Shoes', pl: 'Buty' };
    expect(
      contextDisplayLabel(display, 'id', ['pl-web', 'pl', 'default']),
    ).toBe('Buty');
    expect(contextDisplayLabel(display, 'id', ['de', 'default'])).toBe('Shoes');
    expect(contextDisplayLabel({}, 'id', ['pl'])).toBe('id');
  });
});
