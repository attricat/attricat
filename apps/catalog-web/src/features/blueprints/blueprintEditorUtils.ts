import type { Monaco } from '@monaco-editor/react';
import basicDefinition from './templates/basic.toml?raw';
import productDefinition from './templates/product.toml?raw';
import seoDefinition from './templates/seo.toml?raw';

export const blueprintTemplates = [
  {
    definition: basicDefinition,
    descriptionKey: 'blueprints.basicEntityDescription',
    labelKey: 'blueprints.basicEntity',
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

export const tomlLanguageId = 'toml';

export const configureToml = (monaco: Monaco) => {
  if (
    monaco.languages
      .getLanguages()
      .some((language: { id: string }) => language.id === tomlLanguageId)
  )
    return;
  monaco.languages.register({ id: tomlLanguageId });
  monaco.languages.setMonarchTokensProvider(tomlLanguageId, {
    tokenizer: {
      root: [
        [/^\s*#.*$/, 'comment'],
        [/\[[^\]]+\]/, 'keyword'],
        [/[A-Za-z0-9_-]+(?=\s*=)/, 'type.identifier'],
        [/"([^"\\]|\\.)*"|'([^'\\]|\\.)*'/, 'string'],
        [/\b(true|false)\b/, 'keyword'],
        [/-?\d+(\.\d+)?/, 'number'],
      ],
    },
  });
};
