import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useToast } from '../../components/useToast';
import { deleteLexiconEntry, importLexicon, saveLexiconEntry } from './api';
import type { LexiconImportMode } from './constants';
import { reloadLexicon } from './lexicon';
import { lexiconQueryKeys } from './queryKeys';
import type { LexiconFile } from './schemas';
import { LEXICON_MANAGEMENT_NAMESPACES } from './constants';

/**
 * Lexicon writes. Each success refreshes the management queries and the
 * i18next namespace, so labels across the app update without a reload.
 */
export const useLexiconMutations = () => {
  const { t } = useTranslation(LEXICON_MANAGEMENT_NAMESPACES);
  const client = useQueryClient();
  const { show } = useToast();
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({ queryKey: lexiconQueryKeys.all() }),
      reloadLexicon(),
    ]);
  };
  const save = useMutation({
    mutationFn: saveLexiconEntry,
    meta: { toast: false },
    onSuccess: async () => {
      await refresh();
      show({ message: t('lexicon.translationSaved'), severity: 'success' });
    },
  });
  const remove = useMutation({
    mutationFn: deleteLexiconEntry,
    meta: { toast: false },
    onSuccess: async () => {
      await refresh();
      show({ message: t('lexicon.translationDeleted'), severity: 'success' });
    },
  });
  const importFile = useMutation({
    mutationFn: ({
      file,
      mode,
    }: {
      file: LexiconFile;
      mode: LexiconImportMode;
    }) => importLexicon(file, mode),
    meta: { toast: false },
    onSuccess: async (summary) => {
      await refresh();
      show({ message: t('lexicon.imported', summary), severity: 'success' });
    },
  });
  return { save, remove, importFile };
};
