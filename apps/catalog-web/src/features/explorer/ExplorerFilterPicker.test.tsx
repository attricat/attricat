// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { useState } from 'react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import {
  getBlueprintByCode,
  type BlueprintWithAttributes,
} from '../entities/api';
import { ExplorerFilterPicker } from './ExplorerFilterPicker';
import type { RelationshipFilterAttribute } from './relationshipFilterTypes';
import type { AttributeFilter } from './search';

vi.mock('../entities/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../entities/api')>();
  return { ...actual, getBlueprintByCode: vi.fn() };
});

const relationship = (
  code: string,
  target: string,
): RelationshipFilterAttribute => ({
  cardinality: 'many',
  code,
  target_blueprint_code: target,
  value_type: 'relationship',
});

const blueprint = (
  code: string,
  attributes: RelationshipFilterAttribute[],
): BlueprintWithAttributes => ({
  attributes,
  blueprint: {
    code,
    id: '123e4567-e89b-12d3-a456-426614174000',
    name: code,
    status: 'published',
    version: 1,
    views: {},
  },
  table_path_attributes: [],
});

describe('ExplorerFilterPicker', () => {
  it('discovers relationship paths up to three hops and returns the terminal target', async () => {
    vi.mocked(getBlueprintByCode).mockImplementation((code) => {
      if (code === 'family')
        return Promise.resolve(
          blueprint(code, [relationship('class', 'class')]),
        );
      if (code === 'class')
        return Promise.resolve(blueprint(code, [relationship('kind', 'kind')]));
      return Promise.resolve(blueprint(code, []));
    });
    const onAddRelationship = vi.fn();
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={queryClient}>
        <ExplorerFilterPicker
          attributes={[]}
          blueprintName="Product"
          filters={[]}
          onAdd={vi.fn()}
          onAddRelationship={onAddRelationship}
          onRemove={vi.fn()}
          onUpdate={vi.fn()}
          relationshipAttributes={[relationship('family', 'family')]}
        />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Add filter' }));
    const field = screen.getByRole('combobox', { name: 'Field' });
    await user.click(field);
    const thirdHop = await screen.findByRole('option', {
      name: /family\.class\.kind/i,
    });
    await user.click(thirdHop);

    expect(onAddRelationship).toHaveBeenCalledWith(
      expect.objectContaining({
        code: 'family.class.kind',
        target_blueprint_code: 'kind',
      }),
    );
  });

  it('keeps local attribute names and status options when a table column has the same code', async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={queryClient}>
        <ExplorerFilterPicker
          attributes={[
            {
              code: 'state',
              name: 'Workflow state',
              value_type: 'string',
              value_schema: {
                type: 'string',
                enum: ['draft', 'live'],
                'x-attricat-status': {
                  version: 1,
                  options: [
                    { code: 'draft', label: 'Draft' },
                    { code: 'live', label: 'Live' },
                  ],
                },
              },
            },
          ]}
          blueprintName="Product"
          filters={[]}
          onAdd={vi.fn()}
          onAddRelationship={vi.fn()}
          onRemove={vi.fn()}
          onUpdate={vi.fn()}
          pathAttributes={[{ code: 'state', value_type: 'string' }]}
        />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Add filter' }));
    await user.click(screen.getByRole('combobox', { name: 'Field' }));
    await user.click(
      await screen.findByRole('option', { name: /Workflow state/ }),
    );
    await user.click(screen.getByRole('combobox', { name: 'Value' }));
    expect(await screen.findByRole('option', { name: 'Live' })).toBeTruthy();
  });

  it('keeps keyboard focus in the list when filters are removed', async () => {
    const Picker = () => {
      const [filters, setFilters] = useState<AttributeFilter[]>([
        { field: 'stock', operator: 'gt', value: 5 },
        { field: 'stock', operator: 'lt', value: 50 },
      ]);
      return (
        <ExplorerFilterPicker
          attributes={[{ code: 'stock', value_type: 'integer' }]}
          blueprintName="Product"
          filters={filters}
          onAdd={vi.fn()}
          onAddRelationship={vi.fn()}
          onRemove={(index) =>
            setFilters((current) =>
              current.filter((_, position) => position !== index),
            )
          }
          onUpdate={vi.fn()}
        />
      );
    };
    render(
      <QueryClientProvider client={new QueryClient()}>
        <Picker />
      </QueryClientProvider>,
    );
    const user = userEvent.setup();
    const chips = () =>
      screen
        .getAllByRole('button')
        .filter((element) => element.classList.contains('MuiChip-root'));

    chips()[0].focus();
    await user.keyboard('{Delete}');
    expect(chips()).toHaveLength(1);
    expect(document.activeElement).toBe(chips()[0]);

    await user.keyboard('{Delete}');
    expect(chips()).toHaveLength(0);
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Add filter' }),
    );
  });
});
