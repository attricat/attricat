import { describe, expect, it } from 'vitest';
import { isSafeAutomaticMigration } from './safeMigration';
import type { BlueprintWithAttributes } from './schemas';

const revision = (
  attributes: Record<string, unknown>[],
  entitySchema: unknown = {},
) =>
  ({
    blueprint: { entity_schema: entitySchema },
    attributes: attributes.map((attribute) => ({
      value_type: 'string',
      value_schema: null,
      readonly: false,
      ...attribute,
    })),
  }) as unknown as BlueprintWithAttributes;

describe('isSafeAutomaticMigration', () => {
  it('allows removed attributes and unchanged retained attributes', () => {
    expect(
      isSafeAutomaticMigration(
        revision([{ code: 'name' }, { code: 'sku' }]),
        revision([{ code: 'name' }]),
      ),
    ).toBe(true);
  });

  it('rejects changed attribute contracts and entity schemas', () => {
    expect(
      isSafeAutomaticMigration(
        revision([{ code: 'name' }]),
        revision([{ code: 'name', value_type: 'number' }]),
      ),
    ).toBe(false);
    expect(
      isSafeAutomaticMigration(
        revision([], { required: [] }),
        revision([], { required: ['name'] }),
      ),
    ).toBe(false);
  });
});
