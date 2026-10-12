import { describe, expect, it } from 'vitest';
import type { Attribute, BlueprintWithAttributes } from '../records/api';
import {
  browseByFields,
  defaultBrowseByField,
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
      'product',
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

  it("marks the blueprint's own hierarchy field as its own tree", () => {
    const parent = relationship('parent', 'category', { hierarchy: 'tree' });
    const related = relationship('related', 'category');
    const category = blueprint('category', [parent]);
    expect(
      browseByFields('category', [related, parent], [category, category]),
    ).toEqual([
      { attribute: related, hierarchyField: 'parent' },
      { attribute: parent, hierarchyField: 'parent', ownTree: true },
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

  it("uses the blueprint's default after the person's choices", () => {
    expect(resolveBrowseBy([brand], undefined, undefined, 'brand')).toBe(brand);
    expect(resolveBrowseBy(fields, undefined, 'categories', 'brand')).toBe(
      categories,
    );
    expect(resolveBrowseBy(fields, undefined, false, 'brand')).toBeUndefined();
    expect(
      resolveBrowseBy(fields, undefined, undefined, false),
    ).toBeUndefined();
    expect(resolveBrowseBy(fields, 'brand', undefined, false)).toBe(brand);
    expect(resolveBrowseBy(fields, undefined, undefined, 'removed')).toBe(
      categories,
    );
  });

  it("opens on the blueprint's own tree before a related tree", () => {
    const ownTree: BrowseByField = {
      attribute: relationship('parent', 'product'),
      hierarchyField: 'parent',
      ownTree: true,
    };
    expect(
      resolveBrowseBy([brand, categories, ownTree], undefined, undefined),
    ).toBe(ownTree);
  });

  it('opens by default only for a hierarchy', () => {
    expect(resolveBrowseBy(fields, undefined, undefined)).toBe(categories);
    expect(resolveBrowseBy([brand], undefined, undefined)).toBeUndefined();
  });
});

describe('defaultBrowseByField', () => {
  const brand: BrowseByField = { attribute: relationship('brand', 'brand') };
  const categories: BrowseByField = {
    attribute: relationship('categories', 'category'),
    hierarchyField: 'parent',
  };
  const ownTree: BrowseByField = {
    attribute: relationship('parent', 'product'),
    hierarchyField: 'parent',
    ownTree: true,
  };

  it("prefers the blueprint's default, then its own tree, then a hierarchy", () => {
    const fields = [categories, ownTree, brand];
    expect(defaultBrowseByField(fields, 'brand')).toBe(brand);
    expect(defaultBrowseByField(fields, undefined)).toBe(ownTree);
    expect(defaultBrowseByField(fields, false)).toBe(ownTree);
    expect(defaultBrowseByField([brand, categories], 'removed')).toBe(
      categories,
    );
    expect(defaultBrowseByField([brand], null)).toBeUndefined();
  });
});
