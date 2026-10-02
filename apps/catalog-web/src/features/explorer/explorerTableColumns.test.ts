import { describe, expect, it } from 'vitest';
import type { BlueprintWithAttributes } from '../entities/api';
import { attributeSchema } from '../entities/schemas';
import {
  buildExplorerTableColumns,
  configuredColumnLabel,
} from './explorerTableColumns';

const attributes = [
  attributeSchema.parse({
    code: 'product_family',
    name: 'Family',
    value_type: 'string',
  }),
  attributeSchema.parse({ code: 'unit_price', value_type: 'number' }),
  attributeSchema.parse({
    code: 'brand',
    value_type: 'relationship',
    target_blueprint_code: 'brand',
  }),
];

const blueprintWithTable = (table: unknown) =>
  ({
    blueprint: { views: { table } },
    attributes,
    table_path_attributes: [],
  }) as unknown as BlueprintWithAttributes;

describe('buildExplorerTableColumns', () => {
  it('labels legacy field columns with attribute names', () => {
    const columns = buildExplorerTableColumns(
      blueprintWithTable({
        type: 'table',
        fields: ['product_family', 'unit_price'],
      }),
      true,
    );
    expect(columns.map(configuredColumnLabel)).toEqual([
      'Family',
      'unit price',
    ]);
  });

  it('prefers configured column labels over attribute names', () => {
    const columns = buildExplorerTableColumns(
      blueprintWithTable({
        type: 'table',
        columns: [
          { field: 'product_family' },
          { field: 'product_family', label: 'Line' },
          { field: 'brand.brand_name' },
        ],
      }),
      true,
    );
    expect(columns.map(configuredColumnLabel)).toEqual([
      'Family',
      'Line',
      'brand.brand name',
    ]);
  });
});
