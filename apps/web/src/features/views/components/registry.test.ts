import contract from '../../../../../../contracts/view-components.json';
import { describe, expect, it } from 'vitest';
import { viewComponents } from './registry';

describe('view component registry', () => {
  it('matches the contract used by blueprint validation', () => {
    expect(
      viewComponents.map(
        ({
          id,
          version,
          capabilities,
          placements,
          value_types,
          allowed_props,
        }) => ({
          id,
          version,
          capabilities,
          placements,
          value_types,
          allowed_props,
        }),
      ),
    ).toEqual(contract.components);
  });
});
