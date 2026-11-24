// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { RecordItem } from '../records/api';
import { SearchInfoDialog } from './SearchInfoDialog';

const recordId = '123e4567-e89b-12d3-a456-426614174001';

const record = (explanations: RecordItem['match_explanations']): RecordItem =>
  ({
    id: recordId,
    match_explanations: explanations,
  }) as RecordItem;

describe('SearchInfoDialog', () => {
  it('names the value a fuzzy-search attribute matched by similarity', () => {
    render(
      <SearchInfoDialog
        onClose={vi.fn()}
        record={record([
          {
            term: 'tarcze',
            matching_record_id: recordId,
            matching_attribute_code: 'name',
            similar_value: 'Tarcza tnąca 125 mm',
            traversal_depth: 0,
            relationship_path: [],
          },
          {
            term: 'stal',
            matching_record_id: recordId,
            matching_attribute_code: 'note',
            traversal_depth: 0,
            relationship_path: [],
          },
        ])}
      />,
    );

    expect(
      screen.getByText('tarcze ≈ “Tarcza tnąca 125 mm” in name; stal in note', {
        exact: false,
      }),
    ).toBeTruthy();
  });
});
