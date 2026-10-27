import './monacoRuntime';
import { Editor } from '@monaco-editor/react';
import { Box } from '@mui/material';
import type { ComponentProps } from 'react';
import { minimumTomlEditorHeight } from '../features/blueprints/blueprintEditorUtils';
import { useMonacoTheme } from './monacoTheme';
import { configureToml, tomlLanguageId } from './tomlLanguage';

type EditorProps = ComponentProps<typeof Editor>;

export type TomlEditorProps = {
  ariaLabel?: string;
  height: string;
  onChange: EditorProps['onChange'];
  onMount?: EditorProps['onMount'];
  /** Additional Monaco options, such as suggestion behavior. */
  options?: EditorProps['options'];
  readOnly: boolean;
  marginTop?: number;
  value: string;
};

export const TomlEditor = ({
  ariaLabel,
  height,
  onChange,
  onMount,
  options,
  readOnly,
  marginTop,
  value,
}: TomlEditorProps) => {
  const theme = useMonacoTheme();
  return (
    <Box
      sx={{
        border: 1,
        borderColor: 'divider',
        height,
        minHeight: minimumTomlEditorHeight,
        mt: marginTop,
      }}
    >
      <Editor
        beforeMount={configureToml}
        defaultLanguage={tomlLanguageId}
        height="100%"
        language={tomlLanguageId}
        onChange={onChange}
        onMount={onMount}
        options={{
          automaticLayout: true,
          minimap: { enabled: false },
          scrollBeyondLastLine: false,
          // Keeps the enclosing [table] headers in view while scrolling.
          stickyScroll: { enabled: true, defaultModel: 'outlineModel' },
          tabSize: 2,
          wordWrap: 'on',
          ...options,
          ariaLabel,
          readOnly,
        }}
        theme={theme}
        value={value}
      />
    </Box>
  );
};
