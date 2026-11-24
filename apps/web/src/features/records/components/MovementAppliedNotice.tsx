import { Alert } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../api';
import { attributeLabel } from '../recordDisplay';

/**
 * Explains why a movement record's quantity and links are shown read-only:
 * its movement already applied them, when the record was created or its
 * status changed, so correcting them takes a reversal.
 */
export const MovementAppliedNotice = ({
  attributes,
  inputs,
  saved,
}: {
  /** Attributes shown on the record, to name the applied fields. */
  attributes: readonly Attribute[];
  /** Codes of the attributes the movement read. */
  inputs: ReadonlySet<string>;
  /** Codes of the attributes the record has a value for. */
  saved: ReadonlySet<string>;
}) => {
  const { t, i18n } = useTranslation();
  const fields = attributes
    // An input the record never set, such as an empty reversal link, is not
    // worth naming.
    .filter(
      (attribute) => inputs.has(attribute.code) && saved.has(attribute.code),
    )
    .map(attributeLabel);
  if (fields.length === 0) return null;
  return (
    <Alert severity="info" sx={{ mb: 2 }}>
      {t('records.movementApplied', {
        fields: new Intl.ListFormat(i18n.language, {
          type: 'conjunction',
        }).format(fields),
      })}
    </Alert>
  );
};
