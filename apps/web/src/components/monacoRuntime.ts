import { loader } from '@monaco-editor/react';
import * as monaco from 'monaco-editor/editor';
import EditorWorker from 'monaco-editor/editor/editor.worker?worker';
// Preserve standard editing/accessibility features without bundling every
// language service. TOML uses our providers. All of this stays in editor chunks.
import 'monaco-editor/features/register.all';
import 'monaco-editor/editor/browser/coreCommands';

self.MonacoEnvironment = { getWorker: () => new EditorWorker() };
loader.config({ monaco });
