// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useRef, useState } from 'react';
import { expect, it } from 'vitest';
import '../../i18n';
import {
  ActiveExplorerFilters,
  type ActiveExplorerFilter,
} from './ActiveExplorerFilters';

const Row = () => {
  const queryRef = useRef<HTMLInputElement>(null);
  const [filters, setFilters] = useState<ActiveExplorerFilter[]>([
    {
      filter: { field: 'stock', operator: 'gt', value: 5 },
      index: 0,
      kind: 'attribute',
    },
    { field: 'category', kind: 'relationship', selectedCount: 2 },
  ]);
  return (
    <>
      <input aria-label="Query" ref={queryRef} />
      <ActiveExplorerFilters
        attributes={[{ code: 'stock', value_type: 'integer' }]}
        emptyFocusTarget={queryRef}
        filters={filters}
        onRemoveAttribute={() =>
          setFilters((current) =>
            current.filter((filter) => filter.kind !== 'attribute'),
          )
        }
        onRemoveRelationship={(field) =>
          setFilters((current) =>
            current.filter(
              (filter) =>
                filter.kind !== 'relationship' || filter.field !== field,
            ),
          )
        }
      />
    </>
  );
};

it('keeps keyboard focus on a filter, then the query, as filters are removed', async () => {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <Row />
    </QueryClientProvider>,
  );
  const user = userEvent.setup();
  const chips = () =>
    screen
      .queryAllByRole('button')
      .filter((element) => element.classList.contains('MuiChip-root'));

  chips()[0].focus();
  await user.keyboard('{Delete}');
  expect(chips()).toHaveLength(1);
  expect(document.activeElement).toBe(chips()[0]);

  await user.keyboard('{Delete}');
  expect(chips()).toHaveLength(0);
  expect(document.activeElement).toBe(screen.getByLabelText('Query'));
});
