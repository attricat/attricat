import type { Monaco } from '@monaco-editor/react';
import { useQueryClient } from '@tanstack/react-query';
import type { editor } from 'monaco-editor';
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { YamlEditor, type YamlEditorProps } from '../../components/YamlEditor';
import { diagnosticsDelayMs, type DefinitionKind } from './constants';
import {
  attachDefinitionModel,
  updateDefinitionDiagnostics,
  type DefinitionModelConfig,
} from './definitionLanguage';
import {
  createDefinitionReferences,
  type ReferenceAttribute,
} from './definitionReferences';

const definitionEditorOptions: editor.IStandaloneEditorConstructionOptions = {
  // Most values are strings, so suggest inside them too.
  quickSuggestions: { comments: false, other: true, strings: true },
  suggest: { showWords: false },
  wordBasedSuggestions: 'off',
};

type MountedEditor = { model: editor.ITextModel; monaco: Monaco };

/**
 * A YAML editor that completes, documents, and validates a blueprint,
 * reusable attribute or rule definition from its Rust-derived contract.
 */
export const DefinitionEditor = ({
  kind,
  onMount,
  scopeAttributes,
  value,
  ...props
}: Omit<YamlEditorProps, 'options'> & {
  kind: DefinitionKind;
  /** Attributes the definition refers to without declaring them. */
  scopeAttributes?: readonly ReferenceAttribute[];
}) => {
  const queryClient = useQueryClient();
  const references = useMemo(
    () => ({ ...createDefinitionReferences(queryClient), scopeAttributes }),
    [queryClient, scopeAttributes],
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
    <YamlEditor
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
