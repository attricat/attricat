import type { Monaco } from '@monaco-editor/react';
import type { editor, languages } from 'monaco-editor';
import {
  scanToml,
  type TextRange,
  type TomlHeader,
  type TomlOutline,
  type TomlPath,
} from './tomlOutline';

export const tomlLanguageId = 'toml';

/** Keys whose value names a table in outlines, such as an attribute's code. */
const tableNameKeys = ['code', 'name', 'id'];

const bareKey = '[A-Za-z0-9_-]+';
const basicString = '"(?:[^"\\\\]|\\\\.)*"';
const literalString = "'[^']*'";
const keySegment = `(?:${bareKey}|${basicString}|${literalString})`;

/** TOML 1.0 highlighting that keeps table headers apart from array values. */
export const tomlTokens: languages.IMonarchLanguage = {
  defaultToken: '',
  tokenPostfix: '.toml',
  tokenizer: {
    root: [
      // Headers start a line outside any value; arrays only follow `=`.
      [/^\s*\[\[?/, { token: 'keyword', next: '@header' }],
      { include: '@whitespace' },
      { include: '@key' },
      [/=/, 'delimiter'],
      { include: '@value' },
    ],
    header: [
      [/\]\]?/, { token: 'keyword', next: '@pop' }],
      [/#.*$/, { token: 'comment', next: '@pop' }],
      [/[^\]#]+/, 'keyword'],
    ],
    key: [
      [
        new RegExp(`${keySegment}(?:\\s*\\.\\s*${keySegment})*(?=\\s*=)`),
        'type.identifier',
      ],
    ],
    value: [
      [/"""/, { token: 'string', next: '@multilineBasicString' }],
      [/'''/, { token: 'string', next: '@multilineLiteralString' }],
      [/"/, { token: 'string', next: '@basicString' }],
      [/'[^']*'?/, 'string'],
      [/(?:true|false)\b/, 'keyword'],
      [
        /\d{4}-\d{2}-\d{2}(?:[Tt ]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:[Zz]|[+-]\d{2}:\d{2})?)?/,
        'number',
      ],
      [/\d{2}:\d{2}:\d{2}(?:\.\d+)?/, 'number'],
      [/[+-]?(?:inf|nan)\b/, 'number'],
      [/0x[0-9A-Fa-f_]+|0o[0-7_]+|0b[01_]+/, 'number'],
      [/[+-]?\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d[\d_]*)?/, 'number'],
      [/\[/, { token: 'delimiter.bracket', next: '@array' }],
      [/\{/, { token: 'delimiter.bracket', next: '@inlineTable' }],
      [/,/, 'delimiter'],
    ],
    array: [
      { include: '@whitespace' },
      [/\]/, { token: 'delimiter.bracket', next: '@pop' }],
      { include: '@value' },
    ],
    inlineTable: [
      { include: '@whitespace' },
      [/\}/, { token: 'delimiter.bracket', next: '@pop' }],
      { include: '@key' },
      [/=/, 'delimiter'],
      { include: '@value' },
    ],
    basicString: [
      // Basic strings end with their line even when the quote is missing.
      [/[^"\\]+$/, { token: 'string', next: '@pop' }],
      [/[^"\\]+/, 'string'],
      { include: '@escape' },
      [/"/, { token: 'string', next: '@pop' }],
    ],
    multilineBasicString: [
      [/"""/, { token: 'string', next: '@pop' }],
      [/[^"\\]+/, 'string'],
      [/\\\s*$/, 'string.escape'],
      { include: '@escape' },
      [/"/, 'string'],
    ],
    multilineLiteralString: [
      [/'''/, { token: 'string', next: '@pop' }],
      [/[^']+/, 'string'],
      [/'/, 'string'],
    ],
    escape: [
      [/\\(?:[btnfr"\\]|u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8})/, 'string.escape'],
      [/\\./, 'string.escape.invalid'],
    ],
    whitespace: [
      [/[ \t\r\n]+/, ''],
      [/#.*$/, 'comment'],
    ],
  },
};

const quotePairs = ['"', "'"].map((quote) => ({
  open: quote,
  close: quote,
  notIn: ['string', 'comment'],
}));

export const tomlLanguageConfiguration: languages.LanguageConfiguration = {
  autoClosingPairs: [
    { open: '[', close: ']' },
    { open: '{', close: '}' },
    ...quotePairs,
  ],
  brackets: [
    ['[', ']'],
    ['{', '}'],
  ],
  comments: { lineComment: '#' },
  surroundingPairs: [
    { open: '[', close: ']' },
    { open: '{', close: '}' },
    ...quotePairs.map(({ open, close }) => ({ open, close })),
  ],
  wordPattern: /-?\d*\.\d\w*|[A-Za-z0-9_-]+/,
};

const startsWith = (path: TomlPath, prefix: TomlPath) =>
  path.length > prefix.length &&
  prefix.every((segment, index) => path[index] === segment);

const lineOf = (model: editor.ITextModel, offset: number) =>
  model.getPositionAt(offset).lineNumber;

/** The last non-blank line before `beforeLine`, but not above `fromLine`. */
const lastContentLine = (
  model: editor.ITextModel,
  fromLine: number,
  beforeLine: number,
) => {
  let line = beforeLine - 1;
  while (line > fromLine && !model.getLineContent(line).trim()) line--;
  return line;
};

/** Each header's section ends where the next header, or a non-child one, starts. */
const sectionEnds = (
  model: editor.ITextModel,
  headers: TomlHeader[],
  nested: boolean,
) =>
  headers.map((header, index) => {
    const next = headers
      .slice(index + 1)
      .find(({ path }) => !nested || !startsWith(path, header.path));
    const start = lineOf(model, header.range.start);
    const nextLine = next
      ? lineOf(model, next.range.start)
      : model.getLineCount() + 1;
    return lastContentLine(model, start, nextLine);
  });

/** Folds each table and every array or inline table that spans lines. */
export const tomlFoldingRanges = (
  model: editor.ITextModel,
  outline: TomlOutline,
): languages.FoldingRange[] => {
  const ends = sectionEnds(model, outline.headers, false);
  const toRange = ({ start, end }: TextRange) => ({
    start: lineOf(model, start),
    end: lineOf(model, end),
  });
  return [
    ...outline.headers.map((header, index) => ({
      start: lineOf(model, header.range.start),
      end: ends[index]!,
    })),
    // The closing bracket stays visible, as in other languages.
    ...outline.collections
      .map(toRange)
      .map(({ start, end }) => ({ start, end: end - 1 })),
  ].filter(({ start, end }) => end > start);
};

const formatPath = (path: TomlPath) =>
  path.filter((segment) => typeof segment === 'string').join('.');

const tableName = (outline: TomlOutline, path: TomlPath) => {
  for (const key of tableNameKeys) {
    const value = outline.valueAt([...path, key]);
    if (value !== undefined) return String(value);
  }
  const index = path.at(-1);
  return typeof index === 'number' ? `#${index + 1}` : undefined;
};

/** Tables nested by path, each holding its own keys, for Go to Symbol. */
export const tomlDocumentSymbols = (
  monaco: Monaco,
  model: editor.ITextModel,
  outline: TomlOutline,
): languages.DocumentSymbol[] => {
  const { SymbolKind } = monaco.languages;
  const toRange = (startOffset: number, endLine: number) => {
    const start = model.getPositionAt(startOffset);
    return new monaco.Range(
      start.lineNumber,
      start.column,
      endLine,
      model.getLineMaxColumn(endLine),
    );
  };
  const toSelection = ({ start, end }: TextRange) => {
    const from = model.getPositionAt(start);
    const to = model.getPositionAt(end);
    return new monaco.Range(
      from.lineNumber,
      from.column,
      to.lineNumber,
      to.column,
    );
  };
  const keySymbols = (table: TomlPath) =>
    outline.entries
      .filter(
        ({ path }) =>
          path.length === table.length + 1 &&
          typeof path.at(-1) === 'string' &&
          startsWith(path, table),
      )
      .map(({ path, range }): languages.DocumentSymbol => ({
        detail: '',
        kind: SymbolKind.Property,
        name: String(path.at(-1)),
        range: toSelection(range),
        selectionRange: toSelection(range),
        tags: [],
      }));

  const ends = sectionEnds(model, outline.headers, true);
  const root: languages.DocumentSymbol[] = keySymbols([]);
  const open: { path: TomlPath; symbol: languages.DocumentSymbol }[] = [];
  outline.headers.forEach((header, index) => {
    while (open.length && !startsWith(header.path, open.at(-1)!.path))
      open.pop();
    const symbol: languages.DocumentSymbol = {
      children: keySymbols(header.path),
      detail: tableName(outline, header.path) ?? '',
      kind: header.array ? SymbolKind.Array : SymbolKind.Module,
      name: formatPath(header.path),
      range: toRange(header.range.start, ends[index]!),
      selectionRange: toSelection(header.range),
      tags: [],
    };
    (open.at(-1)?.symbol.children ?? root).push(symbol);
    open.push({ path: header.path, symbol });
  });
  return root;
};

/** Registers TOML highlighting, editing rules, folding, and outline once. */
export const configureToml = (monaco: Monaco) => {
  if (
    monaco.languages
      .getLanguages()
      .some((language: { id: string }) => language.id === tomlLanguageId)
  )
    return;
  monaco.languages.register({ id: tomlLanguageId });
  monaco.languages.setLanguageConfiguration(
    tomlLanguageId,
    tomlLanguageConfiguration,
  );
  monaco.languages.setMonarchTokensProvider(tomlLanguageId, tomlTokens);
  monaco.languages.registerFoldingRangeProvider(tomlLanguageId, {
    provideFoldingRanges: (model: editor.ITextModel) =>
      tomlFoldingRanges(model, scanToml(model.getValue())),
  });
  monaco.languages.registerDocumentSymbolProvider(tomlLanguageId, {
    provideDocumentSymbols: (model: editor.ITextModel) =>
      tomlDocumentSymbols(monaco, model, scanToml(model.getValue())),
  });
};
