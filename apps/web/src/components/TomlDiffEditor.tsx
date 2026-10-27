import './monacoRuntime';
import { DiffEditor } from '@monaco-editor/react';
import { Box, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { useMonacoTheme } from './monacoTheme';
import { configureToml, tomlLanguageId } from './tomlLanguage';

/** Height of a side-by-side definition diff. */
export const TOML_DIFF_EDITOR_HEIGHT = 560;

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

/** Two TOML definitions side by side, read-only, with their changes marked. */
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
  const theme = useMonacoTheme();
  return (
    <Box>
      <Typography sx={{ fontWeight: 700, mb: 1 }}>
        {t('common.diffTitle', {
          original: originalTitle,
          modified: modifiedTitle,
        })}
      </Typography>
      <Box
        sx={{
          border: 1,
          borderColor: 'divider',
          height: TOML_DIFF_EDITOR_HEIGHT,
        }}
      >
        <DiffEditor
          beforeMount={configureToml}
          height="100%"
          language={tomlLanguageId}
          modified={modified}
          options={diffEditorOptions}
          original={original}
          theme={theme}
        />
      </Box>
    </Box>
  );
};
