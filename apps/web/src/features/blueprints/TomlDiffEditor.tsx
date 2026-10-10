import '../../components/monacoRuntime';
import { DiffEditor } from '@monaco-editor/react';
import { Box, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { configureToml, tomlLanguageId } from './blueprintEditorUtils';
import { tomlDiffEditorHeight } from './constants';

const diffEditorOptions = {
  automaticLayout: true,
  domReadOnly: true,
  minimap: { enabled: false },
  originalEditable: false,
  readOnly: true,
  renderSideBySide: true,
  scrollBeyondLastLine: false,
  wordWrap: 'off',
} as const;

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
}) => {
  const { t } = useTranslation();
  return (
    <Box>
      <Typography sx={{ fontWeight: 700, mb: 1 }}>
        {t('blueprints.diffTitle', {
          original: originalTitle,
          modified: modifiedTitle,
        })}
      </Typography>
      <Box
        sx={{ border: 1, borderColor: 'divider', height: tomlDiffEditorHeight }}
      >
        <DiffEditor
          beforeMount={configureToml}
          height="100%"
          language={tomlLanguageId}
          modified={modified}
          options={diffEditorOptions}
          original={original}
          theme="vs"
        />
      </Box>
    </Box>
  );
};
