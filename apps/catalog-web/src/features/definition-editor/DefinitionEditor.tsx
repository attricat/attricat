import type { Monaco } from '@monaco-editor/react';
import { useQueryClient } from '@tanstack/react-query';
import type { editor } from 'monaco-editor';
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { TomlEditor, type TomlEditorProps } from '../../components/TomlEditor';
import { diagnosticsDelayMs, type DefinitionKind } from './constants';
import {
  attachDefinitionModel,
  updateDefinitionDiagnostics,
  type DefinitionModelConfig,
} from './definitionLanguage';
import { createDefinitionReferences } from './definitionReferences';

const definitionEditorOptions: editor.IStandaloneEditorConstructionOptions = {
  // Most values are strings, so suggest inside them too.
  quickSuggestions: { comments: false, other: true, strings: true },
  suggest: { showWords: false },
  wordBasedSuggestions: 'off',
};

type MountedEditor = { model: editor.ITextModel; monaco: Monaco };

/**
 * A TOML editor that completes, documents, and validates a blueprint or
 * reusable attribute definition from its Rust-derived contract.
 */
export const DefinitionEditor = ({
  kind,
  onMount,
  value,
  ...props
}: Omit<TomlEditorProps, 'options'> & { kind: DefinitionKind }) => {
  const queryClient = useQueryClient();
  const references = useMemo(
    () => createDefinitionReferences(queryClient),
    [queryClient],
  );
  // Monaco providers outlive renders, so they read the latest config via a ref.
  const config = useRef<DefinitionModelConfig>({ kind, references });
  useLayoutEffect(() => {
    config.current = { kind, references };
  });
  const [mounted, setMounted] = useState<MountedEditor>();

  useEffect(() => {
    if (!mounted) return;
    return attachDefinitionModel(mounted.monaco, mounted.model, config);
  }, [mounted]);

  useEffect(() => {
    if (!mounted) return;
    const timeout = window.setTimeout(
      () => updateDefinitionDiagnostics(mounted.monaco, mounted.model, kind),
      diagnosticsDelayMs,
    );
    return () => window.clearTimeout(timeout);
  }, [kind, mounted, value]);

  return (
    <TomlEditor
      {...props}
      onMount={(mountedEditor, monaco) => {
        const model = mountedEditor.getModel();
        if (model) setMounted({ model, monaco });
        onMount?.(mountedEditor, monaco);
      }}
      options={definitionEditorOptions}
      value={value}
    />
  );
};
