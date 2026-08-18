import contract from '../component-contract.json';
import { describe, expect, it } from 'vitest';
import { viewComponents } from './registry';

describe('view component registry', () => {
  it('matches the contract used by blueprint validation', () => {
    expect(
      viewComponents.map(
        ({
          valueRenderer: _valueRenderer,
          headingRenderer: _headingRenderer,
          ...component
        }) => component,
      ),
    ).toEqual(contract.components);
  });
});
