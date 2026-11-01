import { TableCell } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { VisuallyHidden } from './VisuallyHidden';

/** The header of a column of row actions: named for screen readers only. */
export const ActionsHeaderCell = () => {
  const { t } = useTranslation();
  return (
    <TableCell>
      <VisuallyHidden>{t('common.actions')}</VisuallyHidden>
    </TableCell>
  );
};
