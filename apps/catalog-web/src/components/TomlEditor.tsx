import { Editor } from '@monaco-editor/react';
import { Box } from '@mui/material';
import type { ComponentProps } from 'react';
import {
  configureToml,
  minimumTomlEditorHeight,
} from '../features/blueprints/blueprint-editor-utils';

type EditorProps = ComponentProps<typeof Editor>;

export const TomlEditor = ({
  ariaLabel,
  height,
  onChange,
  onMount,
  readOnly,
  marginTop,
  value,
}: {
  ariaLabel?: string;
  height: string;
  onChange: EditorProps['onChange'];
  onMount?: EditorProps['onMount'];
  readOnly: boolean;
  marginTop?: number;
  value: string;
}) => (
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
        ariaLabel,
        automaticLayout: true,
        minimap: { enabled: false },
        readOnly,
        scrollBeyondLastLine: false,
        tabSize: 2,
        wordWrap: 'on',
      }}
      value={value}
    />
  </Box>
);
