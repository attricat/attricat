import basicDefinition from './templates/basic.toml?raw';
import productDefinition from './templates/product.toml?raw';
import seoDefinition from './templates/seo.toml?raw';

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

export const minimumTomlEditorHeight = 480;
