import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { scanToml } from '../../components/tomlOutline';
import { definitionKinds } from './constants';
import type { DefinitionReferences } from './definitionReferences';
import {
  referenceAt,
  referenceDeclaration,
  referenceHover,
  referenceLink,
  unknownReferences,
} from './definitionReferenceTargets';

const references: DefinitionReferences = {
  blueprintAttributes: vi.fn(async () => []),
  blueprints: vi.fn(async () => [
    {
      code: 'brand',
      id: 'brand-id',
      kind: 'record',
      name: 'Brand',
      version: 2,
    },
  ]),
  contexts: vi.fn(async () => ['default']),
  roles: vi.fn(async () => ['attricat_manager']),
};

const definition = (target: string) => `format_version = 1
code = "product"
name = "Product"
kind = "record"

[[attributes]]
code = "maker"
value_type = "relationship"
target_blueprint = "${target}"

[views.dropdown_option]
type = "dropdown_option"
fields = ["maker"]
`;

const at = (text: string, value: string) => {
  const outline = scanToml(text);
  const target = referenceAt(
    definitionKinds.blueprint,
    outline,
    text.lastIndexOf(`"${value}"`) + 1,
  );
  return { context: { outline, references }, outline, target: target! };
};

describe('definition reference targets', () => {
  it('describes the blueprint a value names', async () => {
    const text = definition('brand');
    const { context, target } = at(text, 'brand');
    expect(target.reference).toBe('blueprint');
    expect(await referenceHover(context, target)).toEqual([
      '**brand** — Brand',
      'record blueprint, version 2',
    ]);
  });

  it('links blueprints to their page', async () => {
    const { context, target } = at(definition('brand'), 'brand');
    expect(await referenceLink(context, target)).toEqual({
      tooltip: 'Open blueprint in a new tab',
      url: '/manage/blueprints/brand-id',
    });
  });

  it('warns about blueprints the workspace does not have', async () => {
    const text = definition('brnd');
    const warnings = await unknownReferences(definitionKinds.blueprint, {
      outline: scanToml(text),
      references,
    });
    expect(
      warnings.map(({ message, range }) => ({
        message,
        source: text.slice(range.start, range.end),
      })),
    ).toEqual([
      {
        message: '“brnd” does not exist in this workspace.',
        source: '"brnd"',
      },
    ]);
  });

  it('finds attribute declarations in the document', () => {
    const text = definition('brand');
    const { outline, target } = at(text, 'maker');
    const declared = referenceDeclaration(outline, target);
    expect(declared && text.slice(declared.start, declared.end)).toBe(
      '"maker"',
    );
    expect(declared!.start).toBeLessThan(text.indexOf('fields'));
  });
});
