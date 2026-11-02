import './monacoRuntime';
import { Editor } from '@monaco-editor/react';
import { Box } from '@mui/material';
import { useEffect, useRef, useState, type ComponentProps } from 'react';
import { minimumYamlEditorHeight } from '../features/blueprints/blueprintEditorUtils';
import { useMonacoTheme } from './monacoTheme';
import { configureYaml, yamlLanguageId } from './yamlLanguage';

type EditorProps = ComponentProps<typeof Editor>;

export type YamlEditorProps = {
  ariaLabel?: string;
  height: string;
  /** Leave out for a read-only editor. */
  onChange?: EditorProps['onChange'];
  onMount?: EditorProps['onMount'];
  /** Additional Monaco options, such as suggestion behavior. */
  options?: EditorProps['options'];
  readOnly: boolean;
  marginTop?: number;
  value: string;
};

export const YamlEditor = ({
  ariaLabel,
  height,
  onChange,
  onMount,
  options,
  readOnly,
  marginTop,
  value,
}: YamlEditorProps) => {
  const theme = useMonacoTheme();
  // The wrapper replaces the whole text whenever its `value` differs from the
  // editor's, which moves the cursor to the end. A render can carry text older
  // than what was typed since, so it would lose keystrokes on a busy machine.
  // It only gets values from elsewhere, not echoes of what the editor sent.
  const [editorValue, setEditorValue] = useState(value);
  const mounted =
    useRef<Parameters<NonNullable<YamlEditorProps['onMount']>>[0]>(undefined);
  const sent = useRef<string[]>([]);
  const applying = useRef(false);
  useEffect(() => {
    const echoed = sent.current.lastIndexOf(value);
    if (echoed >= 0) {
      sent.current.splice(0, echoed + 1);
      if (mounted.current) return;
    }
    const current = mounted.current;
    const model = current?.getModel();
    if (current && model && current.getValue() !== value) {
      applying.current = true;
      if (readOnly) current.setValue(value);
      else {
        current.executeEdits('', [
          {
            range: model.getFullModelRange(),
            text: value,
            forceMoveMarkers: true,
          },
        ]);
        current.pushUndoStop();
      }
      applying.current = false;
    }
    setEditorValue(value);
  }, [readOnly, value]);
  return (
    <Box
      sx={{
        border: 1,
        borderColor: 'divider',
        height,
        minHeight: minimumYamlEditorHeight,
        mt: marginTop,
      }}
    >
      <Editor
        beforeMount={configureYaml}
        defaultLanguage={yamlLanguageId}
        height="100%"
        language={yamlLanguageId}
        onChange={(next, event) => {
          if (!applying.current && next !== undefined) sent.current.push(next);
          onChange?.(next, event);
        }}
        onMount={(mountedEditor, monaco) => {
          mounted.current = mountedEditor;
          onMount?.(mountedEditor, monaco);
        }}
        options={{
          automaticLayout: true,
          minimap: { enabled: false },
          scrollBeyondLastLine: false,
          // Keeps the enclosing keys in view while scrolling.
          stickyScroll: { enabled: true, defaultModel: 'outlineModel' },
          tabSize: 2,
          wordWrap: 'on',
          ...options,
          ariaLabel,
          readOnly,
        }}
        theme={theme}
        value={editorValue}
      />
    </Box>
  );
};
