// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../i18n';
import { predicateTypes } from './constants';
import { PredicateSummary } from './PredicateSummary';

describe('PredicateSummary', () => {
  it("names a linked record's attributes by code, not by the rule blueprint's labels", () => {
    render(
      <PredicateSummary
        attributes={[
          { code: 'supplier', name: 'Supplier' },
          { code: 'status', name: 'Listing status' },
        ]}
        predicate={{
          type: predicateTypes.linked,
          quantifier: 'all',
          relationship_code: 'supplier',
          predicate: {
            type: predicateTypes.oneOf,
            attribute_code: 'status',
            values: ['active'],
          },
        }}
      />,
    );
    expect(
      screen.getByText('Every record linked through Supplier satisfies:'),
    ).toBeTruthy();
    expect(screen.getByText('status is one of "active"')).toBeTruthy();
  });
});
