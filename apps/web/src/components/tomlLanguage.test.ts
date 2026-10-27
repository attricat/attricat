import type { Monaco } from '@monaco-editor/react';
import type { editor } from 'monaco-editor';
import { describe, expect, it } from 'vitest';
import { tomlDocumentSymbols, tomlFoldingRanges } from './tomlLanguage';
import { scanToml } from './tomlOutline';

/** The parts of a Monaco text model that outline providers use. */
const model = (text: string) => {
  const lines = text.split('\n');
  return {
    getLineContent: (line: number) => lines[line - 1] ?? '',
    getLineCount: () => lines.length,
    getLineMaxColumn: (line: number) => (lines[line - 1] ?? '').length + 1,
    getPositionAt: (offset: number) => {
      const before = text.slice(0, offset).split('\n');
      return {
        column: before.at(-1)!.length + 1,
        lineNumber: before.length,
      };
    },
  } as unknown as editor.ITextModel;
};

const monaco = {
  Range: class {
    constructor(
      readonly startLineNumber: number,
      readonly startColumn: number,
      readonly endLineNumber: number,
      readonly endColumn: number,
    ) {}
  },
  languages: { SymbolKind: { Array: 17, Module: 1, Property: 6 } },
} as unknown as Monaco;

const source = `format_version = 1
code = "product"

[[attributes]]
code = "title"
tags = [
  "a",
  "b",
]

[[attributes]]
code = "price"

[attributes.validation]
min = 0
`;

describe('TOML folding', () => {
  it('folds tables and multi-line arrays', () => {
    expect(tomlFoldingRanges(model(source), scanToml(source))).toEqual([
      { end: 9, start: 4 },
      { end: 12, start: 11 },
      { end: 15, start: 14 },
      { end: 8, start: 6 },
    ]);
  });
});

describe('TOML outline', () => {
  it('nests tables by path and names array tables by their code', () => {
    const symbols = tomlDocumentSymbols(
      monaco,
      model(source),
      scanToml(source),
    );
    const shape = (
      list: typeof symbols,
    ): { children?: unknown; detail: string; name: string }[] =>
      list.map(({ children, detail, name }) => ({
        detail,
        name,
        ...(children?.length ? { children: shape(children) } : {}),
      }));
    expect(shape(symbols)).toEqual([
      { detail: '', name: 'format_version' },
      { detail: '', name: 'code' },
      {
        children: [
          { detail: '', name: 'code' },
          { detail: '', name: 'tags' },
        ],
        detail: 'title',
        name: 'attributes',
      },
      {
        children: [
          { detail: '', name: 'code' },
          {
            children: [{ detail: '', name: 'min' }],
            detail: '',
            name: 'attributes.validation',
          },
        ],
        detail: 'price',
        name: 'attributes',
      },
    ]);
  });
});
