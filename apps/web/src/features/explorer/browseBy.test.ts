import { describe, expect, it } from 'vitest';
import type { Attribute, BlueprintWithAttributes } from '../records/api';
import {
  browseByFields,
  hierarchyFieldOf,
  resolveBrowseBy,
  type BrowseByField,
} from './browseBy';
import type { RelationshipFilterAttribute } from './relationshipFilterTypes';

const blueprint = (
  code: string,
  attributes: Attribute[] = [],
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

const relationship = (
  code: string,
  target: string,
  extra: Pick<Attribute, 'hierarchy' | 'target_blueprint_codes'> = {},
): RelationshipFilterAttribute => ({
  ...extra,
  code,
  target_blueprint_code: target,
  value_type: 'relationship',
});

describe('hierarchyFieldOf', () => {
  it('finds a self-targeting hierarchy, preferring tree over acyclic', () => {
    const category = blueprint('category', [
      relationship('related', 'category'),
      relationship('broader', 'category', { hierarchy: 'acyclic' }),
      relationship('parent', 'category', { hierarchy: 'tree' }),
      relationship('owner', 'team', { hierarchy: 'tree' }),
    ]);
    expect(hierarchyFieldOf(category)).toBe('parent');
  });

  it('ignores self-relationships without a hierarchy', () => {
    expect(
      hierarchyFieldOf(blueprint('brand', [relationship('related', 'brand')])),
    ).toBeUndefined();
  });
});

describe('browseByFields', () => {
  it('lists single-target fields once their targets load', () => {
    const categories = relationship('categories', 'category');
    const brand = relationship('brand', 'brand');
    const related = relationship('related', 'any', {
      target_blueprint_codes: ['product', 'article'],
    });
    const supplier = relationship('supplier', 'supplier');
    const fields = browseByFields(
      [categories, brand, related, supplier],
      [
        blueprint('category', [
          relationship('parent', 'category', { hierarchy: 'tree' }),
        ]),
        blueprint('brand'),
        blueprint('product'),
        undefined,
      ],
    );
    expect(fields).toEqual([
      { attribute: categories, hierarchyField: 'parent' },
      { attribute: brand, hierarchyField: undefined },
    ]);
  });
});

describe('resolveBrowseBy', () => {
  const brand: BrowseByField = { attribute: relationship('brand', 'brand') };
  const categories: BrowseByField = {
    attribute: relationship('categories', 'category'),
    hierarchyField: 'parent',
  };
  const fields = [brand, categories];

  it('prefers the URL, then the stored choice', () => {
    expect(resolveBrowseBy(fields, 'brand', 'categories')).toBe(brand);
    expect(resolveBrowseBy(fields, undefined, 'brand')).toBe(brand);
  });

  it('keeps the rail hidden when that was chosen', () => {
    expect(resolveBrowseBy(fields, false, 'brand')).toBeUndefined();
    expect(resolveBrowseBy(fields, undefined, false)).toBeUndefined();
  });

  it('falls through a field the revision no longer has', () => {
    expect(resolveBrowseBy(fields, 'removed', 'brand')).toBe(brand);
    expect(resolveBrowseBy(fields, 'removed', undefined)).toBe(categories);
  });

  it('opens by default only for a hierarchy', () => {
    expect(resolveBrowseBy(fields, undefined, undefined)).toBe(categories);
    expect(resolveBrowseBy([brand], undefined, undefined)).toBeUndefined();
  });
});
