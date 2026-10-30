import type { Monaco } from '@monaco-editor/react';
import type {
  editor,
  languages,
  MarkerSeverity,
  Position,
} from 'monaco-editor';
import i18n from '../../i18n';
import { yamlLanguageId } from '../../components/yamlLanguage';
import { scanYaml } from '../../components/yamlOutline';
import {
  attributesKey,
  diagnosticsOwner,
  referenceDiagnosticsOwner,
  serverDiagnosticsOwner,
  type DefinitionKind,
} from './constants';
import {
  definitionCompletions,
  suggestionKinds,
  type DefinitionSuggestion,
  type SuggestionKind,
} from './definitionCompletions';
import {
  definitionDiagnostics,
  type DefinitionFix,
} from './definitionDiagnostics';
import { definitionHover } from './definitionHover';
import { declaredNameAt, nameUsages, renameProblem } from './definitionNames';
import type { DefinitionReferences } from './definitionReferences';
import {
  definitionSymbolKinds,
  definitionSymbols,
  type DefinitionSymbol,
  type DefinitionSymbolKind,
} from './definitionSymbols';
import {
  referenceAt,
  referenceDeclaration,
  referenceHover,
  referenceLink,
  referenceValues,
  unknownReferences,
} from './definitionReferenceTargets';
import type { TextRange } from '../../components/yamlOutline';

export type DefinitionModelConfig = {
  kind: DefinitionKind;
  references: DefinitionReferences;
  /**
   * Attribute codes of the revision being revised, which records store
   * values under, so renaming them is refused.
   */
  savedAttributeCodes?: readonly string[];
};

/**
 * Providers are registered once per Monaco instance for the shared YAML
 * language and read each model's current configuration through this map, so
 * plain YAML editors (such as the revision diff) stay unaffected.
 */
const modelConfigs = new WeakMap<
  editor.ITextModel,
  { current: DefinitionModelConfig }
>();
const registeredInstances = new WeakSet<Monaco>();
/** Quick fixes for each model's current client-side markers. */
const modelFixes = new WeakMap<
  editor.ITextModel,
  { fix: DefinitionFix; marker: editor.IMarkerData }[]
>();

const quickFixKind = 'quickfix';

const sameMarker = (left: editor.IMarkerData, right: editor.IMarkerData) =>
  left.message === right.message &&
  left.startLineNumber === right.startLineNumber &&
  left.startColumn === right.startColumn &&
  left.endLineNumber === right.endLineNumber &&
  left.endColumn === right.endColumn;

const triggerCharacters = ['"', "'", '.', '[', ' ', ',', '{'];
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

const symbolKind = (
  monaco: Monaco,
  kind: DefinitionSymbolKind,
): languages.SymbolKind => {
  const kinds = monaco.languages.SymbolKind;
  switch (kind) {
    case definitionSymbolKinds.array:
      return kinds.Array;
    case definitionSymbolKinds.item:
      return kinds.Struct;
    case definitionSymbolKinds.object:
      return kinds.Module;
    case definitionSymbolKinds.property:
      return kinds.Property;
  }
};

const toDocumentSymbol = (
  monaco: Monaco,
  model: editor.ITextModel,
  symbol: DefinitionSymbol,
): languages.DocumentSymbol => ({
  children: symbol.children.map((child) =>
    toDocumentSymbol(monaco, model, child),
  ),
  detail: symbol.detail ?? '',
  kind: symbolKind(monaco, symbol.kind),
  name: symbol.name,
  range: toRange(monaco, model, symbol.range),
  selectionRange: toRange(monaco, model, symbol.selectionRange),
  tags: [],
});

const registerProviders = (monaco: Monaco) => {
  if (registeredInstances.has(monaco)) return;
  registeredInstances.add(monaco);
  monaco.languages.registerCompletionItemProvider(yamlLanguageId, {
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
  monaco.languages.registerCodeActionProvider(yamlLanguageId, {
    provideCodeActions: (
      model: editor.ITextModel,
      _range: unknown,
      context: languages.CodeActionContext,
    ) => {
      const fixes = modelFixes.get(model) ?? [];
      const actions = context.markers.flatMap((marker) =>
        fixes
          .filter((candidate) => sameMarker(candidate.marker, marker))
          .map(({ fix }): languages.CodeAction => ({
            diagnostics: [marker],
            edit: {
              edits: fix.edits.map(({ range, text }) => ({
                resource: model.uri,
                textEdit: { range: toRange(monaco, model, range), text },
                versionId: model.getVersionId(),
              })),
            },
            isPreferred: true,
            kind: quickFixKind,
            title: fix.title,
          })),
      );
      return { actions, dispose: () => {} };
    },
  });
  monaco.languages.registerHoverProvider(yamlLanguageId, {
    provideHover: async (model: editor.ITextModel, position: Position) => {
      const config = modelConfigs.get(model)?.current;
      if (!config) return undefined;
      const text = model.getValue();
      const offset = model.getOffsetAt(position);
      const outline = scanYaml(text);
      const target = referenceAt(config.kind, outline, offset);
      if (target) {
        const contents = await referenceHover(
          { outline, references: config.references },
          target,
        );
        return (
          contents && {
            contents: contents.map((value) => ({ value })),
            range: toRange(monaco, model, target.range),
          }
        );
      }
      const hover = definitionHover(config.kind, text, offset);
      return (
        hover && {
          contents: hover.contents.map((value) => ({ value })),
          range: toRange(monaco, model, hover.range),
        }
      );
    },
  });
  monaco.languages.registerDefinitionProvider(yamlLanguageId, {
    provideDefinition: (model: editor.ITextModel, position: Position) => {
      const config = modelConfigs.get(model)?.current;
      if (!config) return undefined;
      const outline = scanYaml(model.getValue());
      const target = referenceAt(
        config.kind,
        outline,
        model.getOffsetAt(position),
      );
      const declared = target && referenceDeclaration(outline, target);
      return (
        declared && {
          range: toRange(monaco, model, declared),
          uri: model.uri,
        }
      );
    },
  });
  // The declared name at the cursor, where each use of it is, and which use
  // the cursor is on.
  const usagesAt = (model: editor.ITextModel, position: Position) => {
    const config = modelConfigs.get(model)?.current;
    if (!config) return undefined;
    const text = model.getValue();
    const outline = scanYaml(text);
    const offset = model.getOffsetAt(position);
    const declared = declaredNameAt(config.kind, text, outline, offset);
    if (!declared) return undefined;
    const usages = nameUsages(config.kind, text, outline, declared);
    return {
      config,
      declared,
      current:
        usages.find(({ end, start }) => offset >= start && offset <= end) ??
        declared.declaration,
      ranges: usages.map((range) => toRange(monaco, model, range)),
    };
  };
  monaco.languages.registerReferenceProvider(yamlLanguageId, {
    provideReferences: (model: editor.ITextModel, position: Position) =>
      usagesAt(model, position)?.ranges.map((range) => ({
        range,
        uri: model.uri,
      })) ?? [],
  });
  monaco.languages.registerDocumentHighlightProvider(yamlLanguageId, {
    // Elsewhere Monaco keeps highlighting the word under the cursor.
    provideDocumentHighlights: (model: editor.ITextModel, position: Position) =>
      usagesAt(model, position)?.ranges.map((range) => ({ range })),
  });
  monaco.languages.registerRenameProvider(yamlLanguageId, {
    provideRenameEdits: (
      model: editor.ITextModel,
      position: Position,
      newName: string,
    ) => {
      const usages = usagesAt(model, position);
      if (!usages) return undefined;
      const problem = renameProblem(newName);
      if (problem) return { edits: [], rejectReason: problem };
      return {
        edits: usages.ranges.map((range) => ({
          resource: model.uri,
          textEdit: { range, text: newName },
          versionId: model.getVersionId(),
        })),
      };
    },
    resolveRenameLocation: (model: editor.ITextModel, position: Position) => {
      const usages = usagesAt(model, position);
      if (!usages)
        return {
          range: new monaco.Range(
            position.lineNumber,
            position.column,
            position.lineNumber,
            position.column,
          ),
          rejectReason: i18n.t('definitionEditor.rename.nothingToRename'),
          text: '',
        };
      const { config, current, declared } = usages;
      const saved =
        declared.table === attributesKey &&
        config.savedAttributeCodes?.includes(declared.name);
      return {
        range: toRange(monaco, model, current),
        rejectReason: saved
          ? i18n.t('definitionEditor.rename.savedAttribute', {
              code: declared.name,
            })
          : undefined,
        text: declared.name,
      };
    },
  });
  monaco.languages.registerDocumentSymbolProvider(yamlLanguageId, {
    provideDocumentSymbols: (model: editor.ITextModel) =>
      modelConfigs.has(model)
        ? definitionSymbols(model.getValue()).map((symbol) =>
            toDocumentSymbol(monaco, model, symbol),
          )
        : [],
  });
  monaco.languages.registerLinkProvider(yamlLanguageId, {
    provideLinks: async (model: editor.ITextModel) => {
      const config = modelConfigs.get(model)?.current;
      if (!config) return { links: [] };
      const outline = scanYaml(model.getValue());
      const context = { outline, references: config.references };
      const links = await Promise.all(
        referenceValues(config.kind, outline).map(async (target) => {
          const link = await referenceLink(context, target);
          return (
            link && {
              range: toRange(monaco, model, target.range),
              tooltip: link.tooltip,
              url: new URL(link.url, window.location.origin).href,
            }
          );
        }),
      );
      return { links: links.filter((link) => link !== undefined) };
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
    modelFixes.delete(model);
    if (!model.isDisposed()) {
      monaco.editor.setModelMarkers(model, diagnosticsOwner, []);
      monaco.editor.setModelMarkers(model, serverDiagnosticsOwner, []);
      monaco.editor.setModelMarkers(model, referenceDiagnosticsOwner, []);
    }
  };
};

const errorMarker = (
  monaco: Monaco,
  model: editor.ITextModel,
  { message, range }: { message: string; range: TextRange },
  severity: MarkerSeverity = monaco.MarkerSeverity.Error,
): editor.IMarkerData => {
  const { startLineNumber, startColumn, endLineNumber, endColumn } = toRange(
    monaco,
    model,
    range,
  );
  return {
    endColumn,
    endLineNumber,
    message,
    severity,
    startColumn,
    startLineNumber,
  };
};

export const updateDefinitionDiagnostics = (
  monaco: Monaco,
  model: editor.ITextModel,
  kind: DefinitionKind,
) => {
  if (model.isDisposed()) return;
  const located = definitionDiagnostics(kind, model.getValue()).map(
    (diagnostic) => ({
      fix: diagnostic.fix,
      marker: errorMarker(monaco, model, diagnostic),
    }),
  );
  modelFixes.set(
    model,
    located.flatMap(({ fix, marker }) => (fix ? [{ fix, marker }] : [])),
  );
  monaco.editor.setModelMarkers(
    model,
    diagnosticsOwner,
    located.map(({ marker }) => marker),
  );
  void updateReferenceDiagnostics(monaco, model, kind);
};

/** Warns about references to blueprints, roles, or contexts that do not exist. */
const updateReferenceDiagnostics = async (
  monaco: Monaco,
  model: editor.ITextModel,
  kind: DefinitionKind,
) => {
  const config = modelConfigs.get(model)?.current;
  if (!config) return;
  const version = model.getVersionId();
  const unknown = await unknownReferences(kind, {
    outline: scanYaml(model.getValue()),
    references: config.references,
  });
  if (model.isDisposed() || model.getVersionId() !== version) return;
  monaco.editor.setModelMarkers(
    model,
    referenceDiagnosticsOwner,
    unknown.map((diagnostic) =>
      errorMarker(monaco, model, diagnostic, monaco.MarkerSeverity.Warning),
    ),
  );
};

/** Marks where the server's save check failed, or clears the mark. */
export const updateServerDiagnostic = (
  monaco: Monaco,
  model: editor.ITextModel,
  error: { message: string; range: TextRange } | undefined,
) => {
  if (model.isDisposed()) return;
  if (!error) {
    monaco.editor.setModelMarkers(model, serverDiagnosticsOwner, []);
    return;
  }
  monaco.editor.setModelMarkers(model, serverDiagnosticsOwner, [
    errorMarker(monaco, model, error),
  ]);
};
