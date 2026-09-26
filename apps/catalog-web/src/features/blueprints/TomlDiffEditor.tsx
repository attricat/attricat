import { DiffEditor } from '@monaco-editor/react';
import { Box, Typography } from '@mui/material';
import { configureToml } from './blueprintEditorUtils';

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
