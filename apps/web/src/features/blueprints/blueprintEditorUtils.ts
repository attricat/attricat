import basicDefinition from './templates/basic.yaml?raw';
import productDefinition from './templates/product.yaml?raw';
import seoDefinition from './templates/seo.yaml?raw';

export const blueprintTemplates = [
  {
    definition: basicDefinition,
    descriptionKey: 'blueprints.basicRecordDescription',
    labelKey: 'blueprints.basicRecord',
  },
  {
    definition: productDefinition,
    descriptionKey: 'blueprints.productDescription',
    labelKey: 'blueprints.product',
  },
  {
    definition: seoDefinition,
    descriptionKey: 'blueprints.seoMixinDescription',
    labelKey: 'blueprints.seoMixin',
  },
] as const;

export const minimumYamlEditorHeight = 480;

const templateCodeLine = /^code: (\S+)$/m;

/**
 * Renames a template's blueprint code to the first of `code`, `code_2`,
 * `code_3`, … that no blueprint uses, so a new draft can be saved as is.
 */
export const withUnusedBlueprintCode = (
  definition: string,
  takenCodes: ReadonlySet<string>,
) => {
  const base = definition.match(templateCodeLine)?.[1];
  if (!base) return definition;
  let code = base;
  for (let suffix = 2; takenCodes.has(code); suffix += 1)
    code = `${base}_${suffix}`;
  return definition.replace(templateCodeLine, `code: ${code}`);
};
