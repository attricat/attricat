import './monacoRuntime';
import { Editor } from '@monaco-editor/react';
import { Box } from '@mui/material';
import type { ComponentProps } from 'react';
import { minimumYamlEditorHeight } from '../features/blueprints/blueprintEditorUtils';
import { useMonacoTheme } from './monacoTheme';
import { configureYaml, yamlLanguageId } from './yamlLanguage';

type EditorProps = ComponentProps<typeof Editor>;

export type YamlEditorProps = {
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
        onChange={onChange}
        onMount={onMount}
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
        value={value}
      />
    </Box>
  );
};
