import { Button, Chip, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

/** The unsaved marker, Cancel and Save of a definition editor's header. */
export const DefinitionEditorActions = ({
  canSave,
  isDirty,
  isSaving,
  onCancel,
  onSave,
  saveLabel,
}: {
  canSave: boolean;
  isDirty: boolean;
  isSaving: boolean;
  onCancel: () => void;
  onSave: () => void;
  saveLabel: ReactNode;
}) => {
  const { t } = useTranslation();
  return (
    <Stack direction="row" spacing={1}>
      {isDirty && (
        <Chip color="warning" label={t('definitionEditor.unsaved.chip')} />
      )}
      <Button disabled={isSaving} onClick={onCancel}>
        {t('common.cancel')}
      </Button>
      <Button disabled={!canSave} onClick={onSave} variant="contained">
        {saveLabel}
      </Button>
    </Stack>
  );
};

/** Under the editor: how to save, or that a revision needs a change first. */
export const DefinitionSaveHint = ({ unchanged }: { unchanged: boolean }) => {
  const { t } = useTranslation();
  return (
    <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
      {unchanged
        ? t('definitionEditor.makeChangeBeforeSaving')
        : t('definitionEditor.saveShortcut')}
    </Typography>
  );
};

/** Reopens the template choice while writing a new definition. */
export const DefinitionTemplatesButton = ({
  disabled,
  onClick,
}: {
  disabled: boolean;
  onClick: () => void;
}) => {
  const { t } = useTranslation();
  return (
    <Button disabled={disabled} onClick={onClick}>
      {t('definitionEditor.templates.button')}
    </Button>
  );
};
