import { describe, expect, it } from 'vitest';
import { isSafeAutomaticMigration } from './safeMigration';
import type { BlueprintWithAttributes } from './schemas';

const revision = (
  attributes: Record<string, unknown>[],
  recordSchema: unknown = {},
) =>
  ({
    blueprint: { record_schema: recordSchema },
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

  it('rejects a changed generated value format', () => {
    const generated = (format: string) => ({
      format,
      start: 1,
      reset: 'never',
      time_zone: 'UTC',
    });
    expect(
      isSafeAutomaticMigration(
        revision([{ code: 'number', generated: generated('A-{n}') }]),
        revision([{ code: 'number', generated: generated('B-{n}') }]),
      ),
    ).toBe(false);
    expect(
      isSafeAutomaticMigration(
        revision([{ code: 'number', generated: generated('A-{n}') }]),
        revision([{ code: 'number', generated: generated('A-{n}') }]),
      ),
    ).toBe(true);
  });

  it('rejects changed attribute contracts and record schemas', () => {
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
