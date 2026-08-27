import { DiffEditor, type Monaco } from '@monaco-editor/react';
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

export const TomlDiffEditor = ({
  original,
  modified,
  originalTitle,
  modifiedTitle,
}: {
  original: string;
  modified: string;
  originalTitle: string;
  modifiedTitle: string;
}) => (
  <Box>
    <Typography sx={{ fontWeight: 700, mb: 1 }}>
      {originalTitle} → {modifiedTitle}
    </Typography>
    <Box sx={{ border: 1, borderColor: 'divider', height: 560 }}>
      <DiffEditor
        beforeMount={configureToml}
        height="100%"
        language="toml"
        modified={modified}
        options={{
          automaticLayout: true,
          domReadOnly: true,
          minimap: { enabled: false },
          originalEditable: false,
          readOnly: true,
          renderSideBySide: true,
          scrollBeyondLastLine: false,
          wordWrap: 'off',
        }}
        original={original}
        theme="vs"
      />
    </Box>
  </Box>
);
