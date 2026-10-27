import { loader } from '@monaco-editor/react';
import * as monaco from 'monaco-editor/editor';
import EditorWorker from 'monaco-editor/editor/editor.worker?worker';
// Preserve standard editing/accessibility features without bundling every
// language service. TOML uses our providers. All of this stays in editor chunks.
import 'monaco-editor/features/register.all';
import 'monaco-editor/editor/browser/coreCommands';
import {
  monacoBaseThemes,
  monacoThemeColors,
  monacoThemeNames,
} from './monacoTheme';

self.MonacoEnvironment = { getWorker: () => new EditorWorker() };
loader.config({ monaco });

for (const mode of ['light', 'dark'] as const)
  monaco.editor.defineTheme(monacoThemeNames[mode], {
    base: monacoBaseThemes[mode],
    inherit: true,
    rules: [],
    colors: monacoThemeColors(mode),
  });
