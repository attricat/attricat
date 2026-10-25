import { useTranslation } from 'react-i18next';
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
} from '@mui/material';
import type { StoredLexiconEntry } from './schemas';
import { useLexiconMutations } from './useLexiconMutations';
import { LEXICON_MANAGEMENT_NAMESPACES } from './constants';
import { pluralCategoryLabel } from './languages';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

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
    <Dialog
      maxWidth="xs"
      onClose={() => {
        if (!remove.isPending) onClose();
      }}
      open
    >
      <DialogTitle>{t('lexicon.deleteTranslation')}</DialogTitle>
      <DialogContent>
        <DialogContentText>
          {t('lexicon.deleteTranslationDescription', {
            key: entry.key,
            category: pluralCategoryLabel(t, entry.plural_category),
          })}
        </DialogContentText>
        <ApiErrorAlert error={remove.error} sx={{ mt: 2 }} />
      </DialogContent>
      <DialogActions>
        <Button disabled={remove.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          color="error"
          disabled={remove.isPending}
          onClick={() => remove.mutate(entry, { onSuccess: onClose })}
          variant="contained"
        >
          {t('lexicon.delete')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
