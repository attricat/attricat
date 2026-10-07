import { Box, IconButton, Stack, Tooltip, Typography } from '@mui/material';
import { PencilIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { smallIconSize } from '../../../components/iconSizes';
import type { Attribute } from '../api';
import { attributeLabel } from '../entityDisplay';

/**
 * An editable field shown as its value, with a button that opens its editor.
 * The label matches a read-only field's so the layout does not change.
 */
export const FieldValueAtRest = ({
  attribute,
  onEdit,
  children,
}: {
  attribute: Attribute;
  onEdit: () => void;
  children: ReactNode;
}) => {
  const { t } = useTranslation();
  const label = attributeLabel(attribute);
  const editLabel = t('entities.editField', { field: label });
  return (
    <Stack spacing={0.5}>
      <Box sx={{ alignItems: 'center', display: 'flex', gap: 0.5 }}>
        <Typography sx={{ fontWeight: 700 }} variant="subtitle2">
          {label}
        </Typography>
        <Tooltip title={editLabel}>
          <IconButton aria-label={editLabel} onClick={onEdit} size="small">
            <PencilIcon size={smallIconSize} />
          </IconButton>
        </Tooltip>
      </Box>
      {children}
    </Stack>
  );
};
