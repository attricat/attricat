import type { Monaco } from '@monaco-editor/react';

export const blueprintTemplates = [
  {
    definition: `format_version = 1
code = "new_blueprint"
name = "New blueprint"
kind = "entity"

# Optional: edits by these workspace roles retain channel publication.
# [publication]
# retain_on_edit_roles = ["catalog_manager"]

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
`,
    descriptionKey: 'blueprints.basicEntityDescription',
    labelKey: 'blueprints.basicEntity',
  },
  {
    definition: `format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name", "sku"]
separator = " / "

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "sku"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"

[[attributes]]
code = "description"
value_type = "string"

[[attributes]]
code = "product_images"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["jpg", "jpeg", "png", "webp"]
max_bytes = 10485760
purposes = ["product_image"]
image_only = true
`,
    descriptionKey: 'blueprints.productDescription',
    labelKey: 'blueprints.product',
  },
  {
    definition: `format_version = 1
code = "seo"
name = "SEO"
kind = "mixin"

[[attributes]]
code = "meta_title"
value_type = "string"

[[attributes]]
code = "meta_description"
value_type = "string"
`,
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
