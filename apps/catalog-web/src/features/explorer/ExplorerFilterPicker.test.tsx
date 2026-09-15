// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { getBlueprintByCode } from '../entities/api';
import { ExplorerFilterPicker } from './ExplorerFilterPicker';
import type { RelationshipFilterAttribute } from './relationship-filter-types';

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

const blueprint = (code: string, attributes: RelationshipFilterAttribute[]) => ({
  attributes,
  blueprint: { code, name: code, version: 1, views: {} },
  table_path_attributes: [],
});

describe('ExplorerFilterPicker', () => {
  it('discovers relationship paths up to three hops and returns the terminal target', async () => {
    vi.mocked(getBlueprintByCode).mockImplementation((code) => {
      if (code === 'family')
        return Promise.resolve(blueprint(code, [relationship('class', 'class')]));
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
});
