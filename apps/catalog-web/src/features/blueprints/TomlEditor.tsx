import Editor, { type Monaco } from '@monaco-editor/react';
import { Box, Typography } from '@mui/material';

const configureToml = (monaco: Monaco) => {
  if (
    monaco.languages
      .getLanguages()
      .some((language: { id: string }) => language.id === 'toml')
  )
    return;
  monaco.languages.register({ id: 'toml' });
  monaco.languages.setMonarchTokensProvider('toml', {
    tokenizer: {
      root: [
        [/^\s*#.*$/, 'comment'],
        [/\[[^\]]+\]/, 'keyword'],
        [/[A-Za-z0-9_-]+(?=\s*=)/, 'type.identifier'],
        [/"([^"\\]|\\.)*"|'([^'\\]|\\.)*'/, 'string'],
        [/\b(true|false)\b/, 'keyword'],
        [/-?\d+(\.\d+)?/, 'number'],
      ],
    },
  });
};

export const TomlEditor = ({
  title,
  value,
}: {
  title: string;
  value: string;
}) => (
  <Box>
    <Typography sx={{ fontWeight: 700, mb: 1 }}>{title}</Typography>
    <Box sx={{ border: 1, borderColor: 'divider', height: 560 }}>
      <Editor
        beforeMount={configureToml}
        height="100%"
        language="toml"
        options={{
          automaticLayout: true,
          domReadOnly: true,
          lineNumbers: 'on',
          minimap: { enabled: false },
          readOnly: true,
          scrollBeyondLastLine: false,
          wordWrap: 'off',
        }}
        theme="vs"
        value={value}
      />
    </Box>
  </Box>
);
