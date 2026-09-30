import type { Monaco } from '@monaco-editor/react';
import type { editor, languages, Position } from 'monaco-editor';
import { tomlLanguageId } from '../blueprints/blueprintEditorUtils';
import { diagnosticsOwner, type DefinitionKind } from './constants';
import {
  definitionCompletions,
  suggestionKinds,
  type DefinitionSuggestion,
  type SuggestionKind,
} from './definitionCompletions';
import { definitionDiagnostics } from './definitionDiagnostics';
import { definitionHover } from './definitionHover';
import type { DefinitionReferences } from './definitionReferences';
import type { TextRange } from './tomlOutline';

export type DefinitionModelConfig = {
  kind: DefinitionKind;
  references: DefinitionReferences;
};

/**
 * Providers are registered once per Monaco instance for the shared TOML
 * language and read each model's current configuration through this map, so
 * plain TOML editors (such as the revision diff) stay unaffected.
 */
const modelConfigs = new WeakMap<
  editor.ITextModel,
  { current: DefinitionModelConfig }
>();
const registeredInstances = new WeakSet<Monaco>();

const triggerCharacters = ['"', "'", '.', '[', '=', ' ', ',', '{'];
const triggerSuggestCommand = 'editor.action.triggerSuggest';

const completionKind = (
  monaco: Monaco,
  kind: SuggestionKind,
): languages.CompletionItemKind => {
  const kinds = monaco.languages.CompletionItemKind;
  switch (kind) {
    case suggestionKinds.property:
      return kinds.Property;
    case suggestionKinds.reference:
      return kinds.Reference;
    case suggestionKinds.section:
      return kinds.Module;
    case suggestionKinds.variant:
      return kinds.Struct;
    case suggestionKinds.value:
      return kinds.EnumMember;
  }
};

const toRange = (
  monaco: Monaco,
  model: editor.ITextModel,
  range: TextRange,
) => {
  const start = model.getPositionAt(range.start);
  const end = model.getPositionAt(range.end);
  return new monaco.Range(
    start.lineNumber,
    start.column,
    end.lineNumber,
    end.column,
  );
};

const toCompletionItem = (
  monaco: Monaco,
  model: editor.ITextModel,
  suggestion: DefinitionSuggestion,
): languages.CompletionItem => ({
  command: suggestion.triggerSuggest
    ? { id: triggerSuggestCommand, title: '' }
    : undefined,
  detail: suggestion.detail,
  documentation: suggestion.documentation
    ? { value: suggestion.documentation }
    : undefined,
  filterText: suggestion.filterText,
  insertText: suggestion.insertText,
  insertTextRules: suggestion.snippet
    ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
    : undefined,
  kind: completionKind(monaco, suggestion.kind),
  label: suggestion.label,
  range: toRange(monaco, model, suggestion.range),
  sortText: suggestion.sortText,
});

const registerProviders = (monaco: Monaco) => {
  if (registeredInstances.has(monaco)) return;
  registeredInstances.add(monaco);
  monaco.languages.registerCompletionItemProvider(tomlLanguageId, {
    provideCompletionItems: async (
      model: editor.ITextModel,
      position: Position,
    ) => {
      const config = modelConfigs.get(model)?.current;
      if (!config) return { suggestions: [] };
      const suggestions = await definitionCompletions({
        kind: config.kind,
        offset: model.getOffsetAt(position),
        references: config.references,
        text: model.getValue(),
      });
      return {
        suggestions: suggestions.map((suggestion) =>
          toCompletionItem(monaco, model, suggestion),
        ),
      };
    },
    triggerCharacters,
  });
  monaco.languages.registerHoverProvider(tomlLanguageId, {
    provideHover: (model: editor.ITextModel, position: Position) => {
      const config = modelConfigs.get(model)?.current;
      if (!config) return undefined;
      const hover = definitionHover(
        config.kind,
        model.getValue(),
        model.getOffsetAt(position),
      );
      return (
        hover && {
          contents: hover.contents.map((value) => ({ value })),
          range: toRange(monaco, model, hover.range),
        }
      );
    },
  });
};

/**
 * Enables definition completions and hovers for a model. The returned
 * function detaches it and clears its diagnostics.
 */
export const attachDefinitionModel = (
  monaco: Monaco,
  model: editor.ITextModel,
  config: { current: DefinitionModelConfig },
) => {
  registerProviders(monaco);
  modelConfigs.set(model, config);
  return () => {
    modelConfigs.delete(model);
    if (!model.isDisposed())
      monaco.editor.setModelMarkers(model, diagnosticsOwner, []);
  };
};

export const updateDefinitionDiagnostics = (
  monaco: Monaco,
  model: editor.ITextModel,
  kind: DefinitionKind,
) => {
  if (model.isDisposed()) return;
  monaco.editor.setModelMarkers(
    model,
    diagnosticsOwner,
    definitionDiagnostics(kind, model.getValue()).map(({ message, range }) => {
      const { startLineNumber, startColumn, endLineNumber, endColumn } =
        toRange(monaco, model, range);
      return {
        endColumn,
        endLineNumber,
        message,
        severity: monaco.MarkerSeverity.Error,
        startColumn,
        startLineNumber,
      };
    }),
  );
};
