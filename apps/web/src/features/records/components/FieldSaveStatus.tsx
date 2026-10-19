import { Box, Link } from '@mui/material';
import { CheckIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../../components/iconSizes';
import type { FieldSaveState } from '../useRecordFieldSaves';

/**
 * Whether a field's change is being saved or was just saved, with undo, in
 * the top corner of the field so it never moves the layout. A change that
 * failed shows as the field's error instead.
 */
export const FieldSaveStatus = ({
  state,
  onUndo,
}: {
  state?: FieldSaveState;
  onUndo: () => void;
}) => {
  const { t } = useTranslation();
  if (state !== 'saving' && state !== 'saved') return null;
  return (
    <Box
      sx={{
        alignItems: 'center',
        bgcolor: 'background.paper',
        color: 'text.secondary',
        display: 'flex',
        gap: 1,
        position: 'absolute',
        px: 0.5,
        right: 8,
        top: -10,
        typography: 'caption',
        // Above the outline the label also sits on.
        zIndex: 1,
      }}
    >
      {state === 'saving' ? (
        t('records.fieldSaving')
      ) : (
        <>
          <CheckIcon aria-hidden size={compactIconSize} />
          {t('records.fieldSaved')}
          <Link
            component="button"
            onClick={onUndo}
            type="button"
            variant="caption"
          >
            {t('records.undoSave')}
          </Link>
        </>
      )}
    </Box>
  );
};
