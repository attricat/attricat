import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import type { ExplorerSearch } from '../explorer/search';
import { createSavedView } from './api';
import { savedViewQueryKeys } from './queryKeys';
import type { SaveSearchValues } from './SaveSearchDialog';

/**
 * Saves a search as a new saved view and opens it; `onCreated` runs first,
 * such as to close the dialog that asked for it.
 */
export const useCreateSavedView = (onCreated: () => void) => {
  const navigate = useNavigate({ from: '/' });
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({
      search,
      values,
    }: {
      search: ExplorerSearch;
      values: SaveSearchValues;
    }) =>
      createSavedView(
        values.name,
        values.description,
        values.visibility,
        search,
      ),
    onSuccess: (view) => {
      void client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      onCreated();
      void navigate({ to: '/', search: { savedView: view.id } });
    },
  });
};
