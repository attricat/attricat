import { useMutation, useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { DownloadIcon, PlusIcon, UploadIcon } from 'lucide-react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { LexiconIcon } from '../../components/systemIcons';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { supportedLanguages } from '../../i18n';
import {
  exportLexicon,
  getLexiconReport,
  listStoredLexiconEntries,
} from './api';
import { DeleteLexiconEntryDialog } from './DeleteLexiconEntryDialog';
import { ImportLexiconDialog } from './ImportLexiconDialog';
import { canonicalLanguage, languageLabel } from './languages';
import { LexiconCoverageSection } from './LexiconCoverageSection';
import { LexiconEntriesTable } from './LexiconEntriesTable';
import { LexiconEntryDialog } from './LexiconEntryDialog';
import {
  emptyLexiconEntryDraft,
  lexiconReferenceId,
  type LexiconEntryDraft,
} from './entries';
import {
  LEXICON_CONTROL_WIDTH,
  LEXICON_MANAGEMENT_NAMESPACES,
} from './constants';
import { downloadLexiconFile } from './lexiconFile';
import { lexiconQueryKeys } from './queryKeys';
import type { StoredLexiconEntry } from './schemas';

type DialogState =
  | { kind: 'add'; draft: LexiconEntryDraft }
  | { kind: 'edit'; entry: StoredLexiconEntry }
  | { kind: 'delete'; entry: StoredLexiconEntry }
  | { kind: 'import' };

/** Workspace translations for `{{…}}` references in catalog labels. */
export const LexiconPage = ({
  language,
  onLanguageChange,
}: {
  language: string;
  onLanguageChange: (language: string) => void;
}) => {
  const { i18n, t } = useTranslation(LEXICON_MANAGEMENT_NAMESPACES);
  const [filter, setFilter] = useState('');
  const [dialog, setDialog] = useState<DialogState>();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canEdit = session.data?.capabilities?.blueprints_write === true;
  const entries = useQuery({
    queryKey: lexiconQueryKeys.entries(),
    queryFn: ({ signal }) => listStoredLexiconEntries(signal),
  });
  const report = useQuery({
    queryKey: lexiconQueryKeys.report(language),
    queryFn: ({ signal }) => getLexiconReport(language, signal),
  });
  const exportFile = useMutation({
    mutationFn: () => exportLexicon(language),
    // The download is the feedback.
    meta: { toast: false },
    onSuccess: downloadLexiconFile,
  });
  const languageName = languageLabel(language, i18n.language);
  const languages = [
    ...new Set([
      ...supportedLanguages,
      ...(entries.data ?? []).map((entry) => entry.language),
      language,
    ]),
  ].sort();
  const term = filter.trim().toLowerCase();
  const languageEntries = (entries.data ?? []).filter(
    (entry) =>
      entry.language === language &&
      (!term ||
        [entry.key, entry.context ?? '', entry.text].some((value) =>
          value.toLowerCase().includes(term),
        )),
  );
  const unusedReferences = new Set(
    (report.data?.orphaned ?? [])
      .filter((orphan) => orphan.languages.includes(language))
      .map((orphan) => lexiconReferenceId(orphan.key, orphan.context)),
  );
  const coverage = report.data?.languages.find(
    (item) => item.language === language,
  );
  const closeDialog = () => setDialog(undefined);

  return (
    <PageContainer>
      <PageHeader
        actions={
          <Stack direction="row" spacing={1} sx={{ whiteSpace: 'nowrap' }}>
            <Button
              disabled={exportFile.isPending}
              onClick={() => exportFile.mutate()}
              startIcon={<DownloadIcon />}
              variant="outlined"
            >
              {t('lexicon.export')}
            </Button>
            {canEdit && (
              <Button
                onClick={() => setDialog({ kind: 'import' })}
                startIcon={<UploadIcon />}
                variant="outlined"
              >
                {t('lexicon.import')}
              </Button>
            )}
            {canEdit && (
              <Button
                onClick={() =>
                  setDialog({ kind: 'add', draft: emptyLexiconEntryDraft })
                }
                startIcon={<PlusIcon />}
                variant="contained"
              >
                {t('lexicon.addTranslation')}
              </Button>
            )}
          </Stack>
        }
        description={t('lexicon.description')}
        icon={LexiconIcon}
        title={t('lexicon.title')}
      />
      {session.data && !canEdit && (
        <Typography color="text.secondary" sx={{ mt: 2 }} variant="body2">
          {t('lexicon.readOnly')}
        </Typography>
      )}
      <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} sx={{ mt: 3 }}>
        <Autocomplete
          autoSelect
          disableClearable
          freeSolo
          getOptionLabel={(option) => languageLabel(option, i18n.language)}
          onChange={(_, value) => {
            const next = canonicalLanguage(value);
            if (next) onLanguageChange(next);
          }}
          options={languages}
          renderInput={(params) => (
            <TextField {...params} label={t('lexicon.language')} />
          )}
          sx={{ width: { xs: '100%', sm: LEXICON_CONTROL_WIDTH } }}
          value={language}
        />
        <TextField
          label={t('lexicon.filter')}
          onChange={(event) => setFilter(event.target.value)}
          placeholder={t('lexicon.filterPlaceholder')}
          sx={{ width: { xs: '100%', sm: LEXICON_CONTROL_WIDTH } }}
          value={filter}
        />
      </Stack>
      {exportFile.error && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {exportFile.error.message}
        </Alert>
      )}
      {report.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {report.error.message}
        </Alert>
      )}
      {coverage && (
        <LexiconCoverageSection
          canEdit={canEdit}
          coverage={coverage}
          languageName={languageName}
          onTranslate={(draft) => setDialog({ kind: 'add', draft })}
        />
      )}
      {entries.isPending && (
        <Typography sx={{ mt: 3 }}>{t('lexicon.loading')}</Typography>
      )}
      {entries.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {entries.error.message}
        </Alert>
      )}
      {entries.data && (
        <Paper component="section" sx={{ mt: 3 }}>
          <Box sx={{ borderBottom: 1, borderColor: 'divider', p: 2 }}>
            <Typography component="h2" variant="h6">
              {t('lexicon.translationsIn', { language: languageName })}
            </Typography>
            <Typography color="text.secondary" variant="body2">
              {t('lexicon.translationCount', {
                count: languageEntries.length,
              })}
            </Typography>
          </Box>
          {languageEntries.length > 0 ? (
            <LexiconEntriesTable
              canEdit={canEdit}
              entries={languageEntries}
              onDelete={(entry) => setDialog({ kind: 'delete', entry })}
              onEdit={(entry) => setDialog({ kind: 'edit', entry })}
              unusedReferences={unusedReferences}
            />
          ) : (
            <Typography sx={{ p: 2 }}>
              {term
                ? t('lexicon.noMatchingTranslations')
                : t('lexicon.noTranslations', { language: languageName })}
            </Typography>
          )}
        </Paper>
      )}
      {dialog?.kind === 'add' && (
        <LexiconEntryDialog
          initial={dialog.draft}
          language={language}
          mode="add"
          onClose={closeDialog}
        />
      )}
      {dialog?.kind === 'edit' && (
        <LexiconEntryDialog
          initial={{
            key: dialog.entry.key,
            context: dialog.entry.context ?? '',
            plural_category: dialog.entry.plural_category,
            text: dialog.entry.text,
          }}
          language={language}
          mode="edit"
          onClose={closeDialog}
        />
      )}
      {dialog?.kind === 'delete' && (
        <DeleteLexiconEntryDialog entry={dialog.entry} onClose={closeDialog} />
      )}
      {dialog?.kind === 'import' && (
        <ImportLexiconDialog
          onClose={closeDialog}
          onImported={onLanguageChange}
        />
      )}
    </PageContainer>
  );
};
