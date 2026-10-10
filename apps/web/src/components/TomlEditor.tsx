import './monacoRuntime';
import { Editor } from '@monaco-editor/react';
import { Box } from '@mui/material';
import type { ComponentProps } from 'react';
import {
  configureToml,
  minimumTomlEditorHeight,
} from '../features/blueprints/blueprintEditorUtils';

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
}: TomlEditorProps) => (
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
      defaultLanguage="toml"
      height="100%"
      language="toml"
      onChange={onChange}
      onMount={onMount}
      options={{
        automaticLayout: true,
        minimap: { enabled: false },
        scrollBeyondLastLine: false,
        tabSize: 2,
        wordWrap: 'on',
        ...options,
        ariaLabel,
        readOnly,
      }}
      value={value}
    />
  </Box>
);
