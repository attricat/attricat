import { describe, expect, it } from 'vitest';
import type { Attribute } from './api';
import {
  movementInputCodes,
  movementManagedCodes,
  reversalRelationship,
  type Movement,
} from './movements';
import { relationshipScopeFilters } from './components/relationshipScope';

const issue: Movement = {
  code: 'stock',
  direction: 'out',
  quantity: 'quantity',
  factor: { relationship: 'conversion', attribute: 'factor' },
  result: 'base_quantity',
  target: {
    relationship: 'stock_item',
    blueprint: 'stock_item',
    balance: 'quantity',
    key: 'product',
    values: { product: 'product' },
    create_missing: false,
  },
  reversal: { relationship: 'reverses' },
};

const conversion: Attribute = {
  code: 'conversion',
  value_type: 'relationship',
  target_scope: { attribute: 'product', equals: 'product' },
};

describe('movements', () => {
  it('names the attributes a movement sets and reads', () => {
    expect([...movementManagedCodes([issue])]).toEqual([
      'stock_item',
      'base_quantity',
    ]);
    expect([...movementInputCodes([issue])].sort()).toEqual([
      'conversion',
      'product',
      'quantity',
      'reverses',
    ]);
    expect(reversalRelationship([issue])).toBe('reverses');
    expect(movementInputCodes().size).toBe(0);
    // Before a movement applies, its inputs stay editable.
    expect(movementInputCodes([issue], []).size).toBe(0);
    expect(movementInputCodes([issue], ['stock']).has('quantity')).toBe(true);
    expect(reversalRelationship()).toBeUndefined();
  });

  it('counts the link to the record whose status applies a movement', () => {
    const line: Movement = {
      ...issue,
      code: 'reserve',
      reversal: undefined,
      apply_on: [
        { relationship: 'order', attribute: 'status', to: 'approved' },
      ],
    };
    expect(movementInputCodes([line], ['reserve']).has('order')).toBe(true);
  });

  it('filters a scoped picker by the selected record', () => {
    const product = '123e4567-e89b-12d3-a456-426614174000';
    expect(relationshipScopeFilters(conversion, { product })).toEqual([
      { field: 'product', selected_target_ids: [product] },
    ]);
    // Until the other field has a value, every target is offered.
    expect(
      relationshipScopeFilters(conversion, { product: '' }),
    ).toBeUndefined();
    expect(relationshipScopeFilters(conversion, null)).toBeUndefined();
    expect(
      relationshipScopeFilters(
        { ...conversion, target_scope: null },
        { product },
      ),
    ).toBeUndefined();
  });
});
