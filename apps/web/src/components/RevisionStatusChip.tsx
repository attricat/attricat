import { Chip, type ChipProps } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { revisionStatuses, type RevisionStatus } from '../api/revisionStatus';

/** A revision's status: published in green, a draft in amber. */
export const RevisionStatusChip = ({
  size,
  status,
}: {
  size?: ChipProps['size'];
  status: RevisionStatus;
}) => {
  const { t } = useTranslation();
  return (
    <Chip
      color={status === revisionStatuses.published ? 'success' : 'warning'}
      label={t(`common.revisionStatuses.${status}`)}
      size={size}
    />
  );
};
