import { useTranslation } from 'react-i18next';
import { DiscardChangesDialog } from '../drafts/DiscardChangesDialog';

type UnsavedChangesAction = 'replace' | 'discard';

/** Confirms replacing or discarding unsaved definition edits. */
export const UnsavedDefinitionChangesDialog = ({
  action,
  onCancel,
  onConfirm,
}: {
  action: UnsavedChangesAction | undefined;
  onCancel: () => void;
  onConfirm: () => void;
}) => {
  const { t } = useTranslation();
  const replacing = action === 'replace';
  return (
    <DiscardChangesDialog
      description={
        replacing
          ? t('definitionEditor.unsaved.replaceQuestion')
          : t('definitionEditor.unsaved.discardQuestion')
      }
      discardLabel={
        replacing
          ? t('definitionEditor.unsaved.replace')
          : t('definitionEditor.unsaved.discard')
      }
      onDiscard={onConfirm}
      onKeep={onCancel}
      open={action !== undefined}
      title={t('definitionEditor.unsaved.title')}
    />
  );
};
