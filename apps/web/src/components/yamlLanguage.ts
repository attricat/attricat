import type { Monaco } from '@monaco-editor/react';
import type { editor, languages } from 'monaco-editor';
import {
  scanYaml,
  type TextRange,
  type YamlOutline,
  type YamlPath,
} from './yamlOutline';

/** Monaco's built-in YAML language, registered by `monacoRuntime`. */
export const yamlLanguageId = 'yaml';

/** Keys whose value names a mapping in outlines, such as an attribute's code. */
const mappingNameKeys = ['code', 'name', 'id'];

const pathKey = (path: YamlPath) => JSON.stringify(path);

const itemName = (outline: YamlOutline, path: YamlPath) => {
  for (const key of mappingNameKeys) {
    const value = outline.valueAt([...path, key]);
    if (value !== undefined) return String(value);
  }
  return `#${(path.at(-1) as number) + 1}`;
};

/**
 * Block mappings and sequences nested by path, each holding its own keys,
 * for Go to Symbol and sticky scroll. Folding follows indentation.
 */
export const yamlDocumentSymbols = (
  monaco: Monaco,
  model: editor.ITextModel,
  outline: YamlOutline,
): languages.DocumentSymbol[] => {
  const { SymbolKind } = monaco.languages;
  const toMonacoRange = ({ start, end }: TextRange) => {
    const from = model.getPositionAt(start);
    const to = model.getPositionAt(end);
    return new monaco.Range(
      from.lineNumber,
      from.column,
      to.lineNumber,
      to.column,
    );
  };
  const blocks = new Map(
    outline.collections
      .filter(({ flow }) => !flow)
      .map((collection) => [pathKey(collection.path), collection]),
  );
  const symbolsIn = (parent: YamlPath): languages.DocumentSymbol[] =>
    outline.entries
      .filter(
        ({ path }) =>
          path.length === parent.length + 1 &&
          parent.every((segment, index) => path[index] === segment),
      )
      .flatMap(({ path, range }) => {
        const block = blocks.get(pathKey(path));
        const isItem = typeof path.at(-1) === 'number';
        // Scalar items are values, not symbols.
        if (isItem && !block) return [];
        const span = block
          ? { end: block.range.end, start: range.start }
          : range;
        return [
          {
            children: block ? symbolsIn(path) : [],
            detail: '',
            kind: isItem
              ? SymbolKind.Object
              : block?.kind === 'seq'
                ? SymbolKind.Array
                : block
                  ? SymbolKind.Module
                  : SymbolKind.Property,
            name: isItem ? itemName(outline, path) : String(path.at(-1)),
            range: toMonacoRange(span),
            selectionRange: toMonacoRange(range),
            tags: [],
          },
        ];
      });
  return symbolsIn([]);
};

const registeredInstances = new WeakSet<Monaco>();
/** Models whose editor provides an outline of its own. */
const modelsWithOwnOutline = new WeakSet<editor.ITextModel>();

/**
 * Leaves `model`'s outline to another symbol provider, since Monaco lists
 * every provider's symbols. Returns the undo.
 */
export const claimYamlOutline = (model: editor.ITextModel) => {
  modelsWithOwnOutline.add(model);
  return () => modelsWithOwnOutline.delete(model);
};

/** Registers the YAML outline once per Monaco instance. */
export const configureYaml = (monaco: Monaco) => {
  if (registeredInstances.has(monaco)) return;
  registeredInstances.add(monaco);
  monaco.languages.registerDocumentSymbolProvider(yamlLanguageId, {
    provideDocumentSymbols: (model: editor.ITextModel) =>
      modelsWithOwnOutline.has(model)
        ? []
        : yamlDocumentSymbols(monaco, model, scanYaml(model.getValue())),
  });
};
