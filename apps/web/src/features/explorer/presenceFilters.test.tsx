// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import userEvent from '@testing-library/user-event';
import i18n from 'i18next';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { Attribute } from '../records/api';
import { recordSearchFilterSchema } from '../records/schemas';
import { AttributeFilterDialog } from './AttributeFilterDialog';
import {
  attributeFilterLabel,
  filterableValueTypes,
  isFilterableAttribute,
  operatorsForAttribute,
  operatorsForValueType,
} from './attributeFilters';
import {
  attributeFilterInputValue,
  isAttributeFilterValueValid,
  parseAttributeFilterValue,
} from './attributeFilterValues';
import { explorerSearchSchema, inlineExplorerSearchParams } from './search';

vi.mock('../principals/usePrincipalDirectory', () => ({
  usePrincipalDirectory: () => ({ data: { users: [], teams: [] } }),
}));

const filter = { field: 'owner', operator: 'is_set' as const, value: false };
const owner: Attribute = {
  code: 'owner',
  name: 'Owner',
  value_type: 'string',
  value_schema: {
    type: 'string',
    'x-attricat-principal': { version: 1, kinds: ['user', 'team'] },
  },
};

describe('presence filters', () => {
  it.each(filterableValueTypes)(
    'keeps boolean presence separate from %s value parsing',
    (type) => {
      expect(operatorsForValueType(type)).toContain('is_set');
      expect(isAttributeFilterValueValid(type, 'false', 'is_set')).toBe(true);
      expect(isAttributeFilterValueValid(type, '@me', 'is_set')).toBe(false);
      expect(isAttributeFilterValueValid(type, '', 'is_set')).toBe(false);
      expect(parseAttributeFilterValue(type, 'false', 'UTC', 'is_set')).toBe(
        false,
      );
      expect(parseAttributeFilterValue(type, 'true', 'UTC', 'is_set')).toBe(
        true,
      );
      expect(attributeFilterInputValue(filter, type, 'UTC')).toBe('false');
    },
  );

  it('preserves false through saved state and URL serialization, and rejects nonbooleans', () => {
    const state = { blueprint: 'task', attributeFilters: [filter] };
    expect(explorerSearchSchema.parse(state)).toEqual(state);
    expect(
      JSON.parse(inlineExplorerSearchParams(state).get('attributeFilters')!),
    ).toEqual([filter]);
    for (const value of ['false', 0, null]) {
      expect(
        recordSearchFilterSchema.safeParse({ ...filter, value }).success,
      ).toBe(false);
    }
  });

  it('names presence without interpreting false as a principal or status code', async () => {
    await i18n.loadLanguages('pl');
    expect(attributeFilterLabel(i18n.getFixedT('en'), filter, owner)).toBe(
      'Owner is not set',
    );
    expect(
      attributeFilterLabel(
        i18n.getFixedT('en'),
        { ...filter, value: true },
        owner,
      ),
    ).toBe('Owner is set');
    expect(attributeFilterLabel(i18n.getFixedT('pl'), filter, owner)).toBe(
      'Owner nie ma wartości',
    );
  });

  it('offers only presence for file attributes and defaults to it', async () => {
    const photo: Attribute = {
      code: 'photo',
      name: 'Photo',
      value_type: 'file',
    };
    expect(isFilterableAttribute(photo)).toBe(true);
    expect(operatorsForAttribute(photo)).toEqual(['is_set']);
    expect(filterableValueTypes).not.toContain('file');
    const onSubmit = vi.fn();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <AttributeFilterDialog
          attributes={[photo]}
          blueprintName="Product"
          editing={false}
          initialDraft={{ field: 'photo', operator: 'eq', value: '' }}
          maximumReached={false}
          onClose={vi.fn()}
          onSelectRelationship={vi.fn()}
          onSubmit={onSubmit}
          open
          relationshipPathsLoading={false}
        />
      </QueryClientProvider>,
    );
    const user = userEvent.setup();
    expect(screen.getByRole('combobox', { name: 'Operator' }).textContent).toBe(
      'Presence',
    );
    await user.click(screen.getByRole('combobox', { name: 'Operator' }));
    expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual([
      'Presence',
    ]);
    await user.keyboard('{Escape}');
    await user.click(screen.getByRole('combobox', { name: 'Value' }));
    await user.click(screen.getByRole('option', { name: 'Not set' }));
    await user.click(screen.getByRole('button', { name: 'Add filter' }));
    expect(onSubmit).toHaveBeenCalledWith({
      field: 'photo',
      operator: 'is_set',
      value: false,
    });
  });

  it.each([
    owner,
    {
      code: 'observed_at',
      name: 'Observed at',
      value_type: 'datetime',
    } satisfies Attribute,
  ])(
    'edits and submits a missing-value filter for $code using native boolean choices',
    async (attribute) => {
      const onSubmit = vi.fn();
      const client = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      render(
        <QueryClientProvider client={client}>
          <AttributeFilterDialog
            attributes={[attribute]}
            blueprintName="Task"
            editing
            initialDraft={{
              field: attribute.code,
              operator: 'is_set',
              value: 'false',
            }}
            maximumReached={false}
            onClose={vi.fn()}
            onSelectRelationship={vi.fn()}
            onSubmit={onSubmit}
            open
            relationshipPathsLoading={false}
          />
        </QueryClientProvider>,
      );
      const user = userEvent.setup();
      expect(screen.getByRole('combobox', { name: 'Value' }).textContent).toBe(
        'Not set',
      );
      await user.click(screen.getByRole('button', { name: 'Update filter' }));
      expect(onSubmit).toHaveBeenCalledWith({
        field: attribute.code,
        operator: 'is_set',
        value: false,
      });
      await user.click(screen.getByRole('combobox', { name: 'Value' }));
      await user.click(screen.getByRole('option', { name: 'Has a value' }));
      await user.click(screen.getByRole('button', { name: 'Update filter' }));
      expect(onSubmit).toHaveBeenLastCalledWith({
        field: attribute.code,
        operator: 'is_set',
        value: true,
      });
      await user.click(screen.getByRole('combobox', { name: 'Operator' }));
      await user.click(screen.getByRole('option', { name: 'Equals' }));
      expect(
        (
          screen.getByRole('button', {
            name: 'Update filter',
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(true);
    },
  );
});
