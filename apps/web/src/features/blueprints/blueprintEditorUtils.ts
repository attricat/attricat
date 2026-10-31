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
