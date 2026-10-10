import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  FormControlLabel,
  Radio,
  RadioGroup,
  Stack,
  Typography,
} from '@mui/material';
import { UploadIcon } from 'lucide-react';
import {
  LEXICON_FILE_ACCEPT,
  LEXICON_IMPORT_MODES,
  type LexiconImportMode,
  LEXICON_MANAGEMENT_NAMESPACES,
} from './constants';
import { canonicalLanguage, languageLabel } from './languages';
import { parseLexiconFile } from './lexiconFile';
import type { LexiconFile } from './schemas';
import { useLexiconMutations } from './useLexiconMutations';

/** Imports a JSON or TOML lexicon file, the format `acli lexicon export` writes. */
export const ImportLexiconDialog = ({
  onClose,
  onImported,
}: {
  onClose: () => void;
  onImported: (language: string) => void;
}) => {
  const { i18n, t } = useTranslation(LEXICON_MANAGEMENT_NAMESPACES);
  const { importFile } = useLexiconMutations();
  const [file, setFile] = useState<{ name: string; content: LexiconFile }>();
  const [fileError, setFileError] = useState<string>();
  const [mode, setMode] = useState<LexiconImportMode>('merge');
  const language = file && canonicalLanguage(file.content.language);
  const readFile = async (selected: File | undefined) => {
    setFile(undefined);
    setFileError(undefined);
    if (!selected) return;
    try {
      const content = parseLexiconFile(selected.name, await selected.text());
      if (!canonicalLanguage(content.language))
        throw new Error(t('lexicon.invalidLanguage'));
      setFile({ name: selected.name, content });
    } catch {
      setFileError(t('lexicon.invalidFile'));
    }
  };
  return (
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={() => {
        if (!importFile.isPending) onClose();
      }}
      open
    >
      <DialogTitle>{t('lexicon.importTranslations')}</DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          <DialogContentText>
            {t('lexicon.importDescription')}
          </DialogContentText>
          <Stack
            direction="row"
            spacing={1.5}
            sx={{ alignItems: 'center', flexWrap: 'wrap' }}
          >
            <Button
              component="label"
              disabled={importFile.isPending}
              startIcon={<UploadIcon />}
              variant="outlined"
            >
              {t('lexicon.chooseFile')}
              <input
                accept={LEXICON_FILE_ACCEPT}
                hidden
                onChange={(event) => void readFile(event.target.files?.[0])}
                type="file"
              />
            </Button>
            {file && language && (
              <Typography variant="body2">
                {t('lexicon.selectedFile', {
                  name: file.name,
                  count: file.content.entries.length,
                  language: languageLabel(language, i18n.language),
                })}
              </Typography>
            )}
          </Stack>
          {fileError && <Alert severity="error">{fileError}</Alert>}
          <RadioGroup
            onChange={(event) =>
              setMode(event.target.value as LexiconImportMode)
            }
            value={mode}
          >
            {LEXICON_IMPORT_MODES.map((option) => (
              <FormControlLabel
                control={<Radio />}
                key={option}
                label={t(`lexicon.importModes.${option}`)}
                value={option}
              />
            ))}
          </RadioGroup>
          {importFile.error && (
            <Alert severity="error">{importFile.error.message}</Alert>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button disabled={importFile.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          color={mode === 'replace' ? 'error' : 'primary'}
          disabled={!file || !language || importFile.isPending}
          onClick={() => {
            if (!file || !language) return;
            importFile.mutate(
              { file: { ...file.content, language }, mode },
              {
                onSuccess: () => {
                  onImported(language);
                  onClose();
                },
              },
            );
          }}
          variant="contained"
        >
          {t('lexicon.import')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
