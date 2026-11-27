import { loader } from '@monaco-editor/react';
import * as monaco from 'monaco-editor/editor';
import EditorWorker from 'monaco-editor/editor/editor.worker?worker';
import JsonWorker from 'monaco-editor/languages/features/json/json.worker?worker';
// Preserve standard editing/accessibility features without bundling every
// language service. YAML uses our providers. All of this stays in editor chunks.
import 'monaco-editor/features/register.all';
// Highlighting, brackets and indentation folding for definitions.
import 'monaco-editor/languages/definitions/yaml/register';
// Highlighting, validation and formatting for JSON such as extension
// configuration; its language service runs in its own worker.
import 'monaco-editor/languages/features/json/register';
import 'monaco-editor/editor/browser/coreCommands';
import { jsonLanguageId } from './jsonLanguage';
import {
  monacoBaseThemes,
  monacoThemeColors,
  monacoThemeNames,
  monacoThemeRules,
} from './monacoTheme';

self.MonacoEnvironment = {
  getWorker: (_, label) =>
    label === jsonLanguageId ? new JsonWorker() : new EditorWorker(),
};
loader.config({ monaco });

for (const mode of ['light', 'dark'] as const)
  monaco.editor.defineTheme(monacoThemeNames[mode], {
    base: monacoBaseThemes[mode],
    inherit: true,
    rules: monacoThemeRules(mode),
    colors: monacoThemeColors(mode),
  });
