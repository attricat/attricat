import { Box, Link } from '@mui/material';
import { CheckIcon } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../../components/iconSizes';
import { focusControlIn } from '../../../components/initialFocus';
import { fieldSaveStates, type FieldSaveState } from '../constants';

/**
 * Whether a field's change is being saved or was just saved, with undo, in
 * the top corner of the field so it never moves the layout. A change that
 * failed shows as the field's error instead.
 */
export const FieldSaveStatus = ({
  state,
  onHold,
  onUndo,
  undoFields,
}: {
  state?: FieldSaveState;
  /** Keeps undo available while the user points at or focuses it. */
  onHold: (held: boolean) => void;
  onUndo: () => void;
  /** Names of every field the undo puts back. */
  undoFields: readonly string[];
}) => {
  const { t, i18n } = useTranslation();
  const [focused, setFocused] = useState(false);
  const [hovered, setHovered] = useState(false);
  // The link can go while focused or pointed at, without a blur or leave.
  if (state !== fieldSaveStates.saved && (focused || hovered)) {
    setFocused(false);
    setHovered(false);
  }
  const holding = state === fieldSaveStates.saved && (focused || hovered);
  useEffect(() => {
    if (!holding) return;
    onHold(true);
    return () => onHold(false);
  }, [holding, onHold]);
  if (state !== fieldSaveStates.saving && state !== fieldSaveStates.saved)
    return null;
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
      {state === fieldSaveStates.saving ? (
        t('records.fieldSaving')
      ) : (
        <>
          <CheckIcon aria-hidden size={compactIconSize} />
          {t('records.fieldSaved')}
          <Link
            aria-label={t('records.undoSaveOf', {
              fields: new Intl.ListFormat(i18n.language).format(undoFields),
            })}
            component="button"
            onBlur={() => setFocused(false)}
            onClick={(event) => {
              // The link gives way to the saving status; keep focus on the
              // field it belongs to.
              const field = event.currentTarget.closest('[data-field-code]');
              onUndo();
              focusControlIn(field);
            }}
            onFocus={() => setFocused(true)}
            onMouseEnter={() => setHovered(true)}
            onMouseLeave={() => setHovered(false)}
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
