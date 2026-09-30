import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import type { EntityItem } from '../entities/api';
import { entitySelectionSearch, type ExplorerSearch } from '../explorer/search';
import { createSavedView } from './api';
import { savedViewQueryKeys } from './queryKeys';
import { SaveSearchDialog, type SaveSearchValues } from './SaveSearchDialog';

type Props = {
  entities: EntityItem[];
  onClose: () => void;
  onSuccess: () => void;
  search: ExplorerSearch;
};

/** Saves a search that matches exactly the selected entities by ID. */
export const SaveSelectionAsSearchDialog = ({
  entities,
  onClose,
  onSuccess,
  search,
}: Props) => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/' });
  const client = useQueryClient();
  const save = useMutation({
    mutationFn: ({ description, name, visibility }: SaveSearchValues) =>
      createSavedView(
        name,
        description,
        visibility,
        entitySelectionSearch(
          search,
          entities.map((entity) => entity.id),
        ),
      ),
    onSuccess: (view) => {
      void client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      onSuccess();
      void navigate({ to: '/', search: { savedView: view.id } });
    },
  });

  return (
    <SaveSearchDialog
      error={save.error?.message ?? ''}
      isPending={save.isPending}
      onClose={onClose}
      onSave={(values) => save.mutate(values)}
      open
      title={t('explorer.saveSelectionAsSearch')}
    />
  );
};
