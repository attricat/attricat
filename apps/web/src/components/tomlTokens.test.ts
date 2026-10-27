// @vitest-environment jsdom
import { beforeAll, describe, expect, it } from 'vitest';
import { tomlLanguageId, tomlTokens } from './tomlLanguage';

type MonacoApi = typeof import('monaco-editor');
let monaco: MonacoApi;

/** Loading Monaco takes a few seconds, more while other suites run. */
const monacoLoadTimeoutMs = 60_000;

beforeAll(async () => {
  // Monaco's theme service watches the colour scheme, which jsdom lacks.
  window.matchMedia = (() => ({
    addEventListener: () => {},
    matches: false,
    removeEventListener: () => {},
  })) as unknown as typeof window.matchMedia;
  monaco =
    (await import('monaco-editor/editor/editor.api')) as unknown as MonacoApi;
  monaco.languages.register({ id: tomlLanguageId });
  monaco.languages.setMonarchTokensProvider(tomlLanguageId, tomlTokens);
}, monacoLoadTimeoutMs);

/** Each line's first token type, without the language postfix. */
const firstTokens = (text: string) =>
  monaco.editor
    .tokenize(text, tomlLanguageId)
    .map((line) => line[0]?.type.replace(`.${tomlLanguageId}`, ''));

describe('TOML highlighting', () => {
  it('ends an unclosed table header with its line', () => {
    expect(firstTokens('[attributes\ncode = "title"')).toEqual([
      'keyword',
      'type.identifier',
    ]);
  });

  it('ends a lone opening bracket with its line', () => {
    expect(firstTokens('[[\ncode = "title"')).toEqual([
      'keyword',
      'type.identifier',
    ]);
  });
});
