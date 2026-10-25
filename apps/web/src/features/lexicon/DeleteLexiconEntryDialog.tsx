import { useTranslation } from 'react-i18next';
import type { StoredLexiconEntry } from './schemas';
import { useLexiconMutations } from './useLexiconMutations';
import { LEXICON_MANAGEMENT_NAMESPACES } from './constants';
import { pluralCategoryLabel } from './languages';
import { ConfirmDialog } from '../../components/ConfirmDialog';

export const DeleteLexiconEntryDialog = ({
  entry,
  onClose,
}: {
  entry: StoredLexiconEntry;
  onClose: () => void;
}) => {
  const { t } = useTranslation(LEXICON_MANAGEMENT_NAMESPACES);
  const { remove } = useLexiconMutations();
  return (
    <ConfirmDialog
      confirmLabel={t('lexicon.delete')}
      destructive
      error={remove.error}
      isPending={remove.isPending}
      onClose={onClose}
      onConfirm={() => remove.mutate(entry, { onSuccess: onClose })}
      title={t('lexicon.deleteTranslation')}
    >
      {t('lexicon.deleteTranslationDescription', {
        key: entry.key,
        category: pluralCategoryLabel(t, entry.plural_category),
      })}
    </ConfirmDialog>
  );
};
