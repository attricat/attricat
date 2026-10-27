import { useTranslation } from 'react-i18next';
import type { RecordItem } from '../records/api';
import { recordSelectionSearch, type ExplorerSearch } from '../explorer/search';
import { SaveSearchDialog } from './SaveSearchDialog';
import { useCreateSavedView } from './useCreateSavedView';

type Props = {
  records: RecordItem[];
  onClose: () => void;
  onSuccess: () => void;
  search: ExplorerSearch;
};

/** Saves a search that matches exactly the selected records by ID. */
export const SaveSelectionAsSearchDialog = ({
  records,
  onClose,
  onSuccess,
  search,
}: Props) => {
  const { t } = useTranslation();
  const save = useCreateSavedView(onSuccess);

  return (
    <SaveSearchDialog
      error={save.error}
      isPending={save.isPending}
      onClose={onClose}
      onSave={(values) =>
        save.mutate({
          search: recordSelectionSearch(
            search,
            records.map((record) => record.id),
          ),
          values,
        })
      }
      title={t('explorer.saveSelectionAsSearch')}
    />
  );
};
