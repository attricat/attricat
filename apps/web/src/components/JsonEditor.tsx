import './monacoRuntime';
import { jsonLanguageId } from './jsonLanguage';
import { Editor } from '@monaco-editor/react';
import { Box, FormHelperText, FormLabel } from '@mui/material';
import { useEffect, useId, useRef, useState, type ComponentProps } from 'react';
import { jsonEditorMaxHeight, jsonEditorMinHeight } from './jsonEditorSize';
import { useMonacoTheme } from './monacoTheme';

type OnMount = NonNullable<ComponentProps<typeof Editor>['onMount']>;

const jsonIndentation = 2;
const formatDocumentAction = 'editor.action.formatDocument';

export type JsonEditorProps = {
  /** The starting text; remount the editor to load other text. */
  defaultValue: string;
  /** Shown below the editor and marks it as invalid. */
  error?: string;
  label: string;
  onBlur?: () => void;
  onChange: (value: string) => void;
  readOnly?: boolean;
};

/**
 * A JSON editor with syntax colors and validation. It formats what is
 * pasted or typed, and the whole document when it loses focus.
 */
export const JsonEditor = ({
  defaultValue,
  error,
  label,
  onBlur,
  onChange,
  readOnly = false,
}: JsonEditorProps) => {
  const theme = useMonacoTheme();
  const labelId = useId();
  const [height, setHeight] = useState(jsonEditorMinHeight);
  // Monaco keeps the listeners registered at mount, so they read these.
  const latest = useRef({ onBlur, readOnly });
  useEffect(() => {
    latest.current = { onBlur, readOnly };
  }, [onBlur, readOnly]);
  const onMount: OnMount = (mounted) => {
    const fit = () =>
      setHeight(
        Math.min(
          jsonEditorMaxHeight,
          Math.max(jsonEditorMinHeight, mounted.getContentHeight()),
        ),
      );
    mounted.onDidContentSizeChange(fit);
    fit();
    mounted.onDidBlurEditorText(() => {
      // Formatting is an edit, so it stays on the undo stack.
      if (!latest.current.readOnly)
        void mounted.getAction(formatDocumentAction)?.run();
      latest.current.onBlur?.();
    });
  };
  return (
    <Box>
      <FormLabel error={Boolean(error)} id={labelId}>
        {label}
      </FormLabel>
      <Box
        aria-labelledby={labelId}
        role="group"
        sx={{
          border: 1,
          borderColor: error ? 'error.main' : 'divider',
          height,
          mt: 1,
        }}
      >
        <Editor
          defaultLanguage={jsonLanguageId}
          defaultValue={defaultValue}
          height="100%"
          onChange={(next) => onChange(next ?? '')}
          onMount={onMount}
          options={{
            ariaLabel: label,
            automaticLayout: true,
            formatOnPaste: true,
            formatOnType: true,
            minimap: { enabled: false },
            readOnly,
            scrollBeyondLastLine: false,
            tabSize: jsonIndentation,
            wordWrap: 'on',
          }}
          theme={theme}
        />
      </Box>
      {error && <FormHelperText error>{error}</FormHelperText>}
    </Box>
  );
};
