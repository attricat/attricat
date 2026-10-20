import { describe, expect, it } from 'vitest';
import type { Attribute, BlueprintWithAttributes } from '../records/api';
import {
  browseByFields,
  browsePathFields,
  defaultBrowseByField,
  extendBrowsePaths,
  hierarchyFieldOf,
  resolveBrowseBy,
  type BrowseByField,
} from './browseBy';
import { maximumBrowsePaths } from './constants';
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
  extra: Pick<
    Attribute,
    'cardinality' | 'hierarchy' | 'name' | 'tags' | 'target_blueprint_codes'
  > = {},
): RelationshipFilterAttribute => ({
  ...extra,
  code,
  target_blueprint_code: target,
  value_type: 'relationship',
});

const lookup =
  (...blueprints: BlueprintWithAttributes[]) =>
  (code: string) =>
    blueprints.find((candidate) => candidate.blueprint.code === code);

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
      lookup(
        blueprint('category', [
          relationship('parent', 'category', { hierarchy: 'tree' }),
        ]),
        blueprint('brand'),
        blueprint('product'),
      ),
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
      browseByFields('category', [related, parent], lookup(category)),
    ).toEqual([
      { attribute: related, hierarchyField: 'parent' },
      { attribute: parent, hierarchyField: 'parent', ownTree: true },
    ]);
  });
});

describe('browse paths', () => {
  const model = relationship('model', 'model', {
    cardinality: 'one',
    name: 'Model',
  });
  const brand = relationship('brand', 'brand', {
    cardinality: 'one',
    name: 'Brand',
  });
  const bodyType = relationship('body_type', 'body_type', {
    cardinality: 'one',
    name: 'Body type',
  });
  const group = relationship('group', 'group', {
    cardinality: 'one',
    name: 'Group',
  });
  const blueprints = lookup(
    blueprint('model', [
      brand,
      bodyType,
      // Several values, several targets, hidden, or back to a passed blueprint.
      relationship('options', 'option', { cardinality: 'many' }),
      relationship('related', 'any', {
        cardinality: 'one',
        target_blueprint_codes: ['brand', 'group'],
      }),
      relationship('internal', 'brand', {
        cardinality: 'one',
        tags: ['hidden'],
      }),
      relationship('flagship', 'vehicle', { cardinality: 'one' }),
      relationship('successor', 'model', { cardinality: 'one' }),
      relationship('name_brand', 'brand'),
    ]),
    blueprint('brand', [group, relationship('model', 'model', model)]),
    blueprint('body_type', [
      relationship('parent', 'body_type', {
        cardinality: 'one',
        hierarchy: 'tree',
      }),
    ]),
    blueprint('group', [
      relationship('owner', 'company', { cardinality: 'one' }),
    ]),
    blueprint('company'),
  );

  it('follows single-valued, single-target hops up to three deep', () => {
    const twoHops = extendBrowsePaths(
      'vehicle',
      [[model], [relationship('options', 'option')]],
      blueprints,
    );
    expect(twoHops).toEqual([
      [model, brand],
      [model, bodyType],
    ]);
    const threeHops = extendBrowsePaths('vehicle', twoHops, blueprints);
    expect(threeHops).toEqual([[model, brand, group]]);
    expect(extendBrowsePaths('vehicle', threeHops, blueprints)).toEqual([]);
  });

  it('labels paths by their hops and lists trees as trees', () => {
    expect(
      browsePathFields(
        [
          [model, brand, group],
          [model, brand],
          [model, bodyType],
        ],
        blueprints,
      ),
    ).toEqual([
      {
        attribute: {
          cardinality: 'one',
          code: 'model.brand',
          name: 'Model → Brand',
          target_blueprint_code: 'brand',
          value_type: 'relationship',
        },
        hierarchyField: undefined,
        hops: [model, brand],
      },
      {
        attribute: {
          cardinality: 'one',
          code: 'model.body_type',
          name: 'Model → Body type',
          target_blueprint_code: 'body_type',
          value_type: 'relationship',
        },
        hierarchyField: 'parent',
        hops: [model, bodyType],
      },
      {
        attribute: {
          cardinality: 'one',
          code: 'model.brand.group',
          name: 'Model → Brand → Group',
          target_blueprint_code: 'group',
          value_type: 'relationship',
        },
        hierarchyField: undefined,
        hops: [model, brand, group],
      },
    ]);
  });

  it('bounds the list but keeps a chosen path', () => {
    const many = Array.from({ length: maximumBrowsePaths + 2 }, (_, index) => [
      model,
      relationship(`brand_${index}`, 'brand', { cardinality: 'one' }),
    ]);
    const fields = browsePathFields(many, blueprints, [
      `model.brand_${maximumBrowsePaths + 1}`,
      undefined,
      false,
    ]);
    expect(fields.map((field) => field.attribute.code)).toEqual([
      ...many
        .slice(0, maximumBrowsePaths)
        .map((path) => `model.${path[1].code}`),
      `model.brand_${maximumBrowsePaths + 1}`,
    ]);
  });

  it('opens on a path only when the blueprint names it', () => {
    const fields = browsePathFields([[model, bodyType]], blueprints);
    expect(defaultBrowseByField(fields, undefined)).toBeUndefined();
    expect(defaultBrowseByField(fields, 'model.body_type')).toBe(fields[0]);
    expect(resolveBrowseBy(fields, 'model.body_type', undefined)).toBe(
      fields[0],
    );
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
