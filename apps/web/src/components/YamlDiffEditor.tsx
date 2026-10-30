import './monacoRuntime';
import { DiffEditor } from '@monaco-editor/react';
import { Box, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { useMonacoTheme } from './monacoTheme';
import { configureYaml, yamlLanguageId } from './yamlLanguage';

/** Height of a side-by-side definition diff. */
export const YAML_DIFF_EDITOR_HEIGHT = 560;

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

/** Two YAML definitions side by side, read-only, with their changes marked. */
export const YamlDiffEditor = ({
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
          height: YAML_DIFF_EDITOR_HEIGHT,
        }}
      >
        <DiffEditor
          beforeMount={configureYaml}
          height="100%"
          language={yamlLanguageId}
          modified={modified}
          options={diffEditorOptions}
          original={original}
          theme={theme}
        />
      </Box>
    </Box>
  );
};
