import { Chip } from '@mui/material';
import {
  CircleAlertIcon,
  CircleCheckIcon,
  CircleHelpIcon,
  CircleXIcon,
  ClockIcon,
  RotateCcwIcon,
  UndoIcon,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ExportStatus } from './constants';

const statusChips = {
  pending: { color: 'default', icon: ClockIcon },
  retrying: { color: 'warning', icon: RotateCcwIcon },
  uncertain: { color: 'warning', icon: CircleHelpIcon },
  rejected: { color: 'error', icon: CircleXIcon },
  synced: { color: 'success', icon: CircleCheckIcon },
  withdrawn: { color: 'default', icon: UndoIcon },
} as const satisfies Record<ExportStatus, unknown>;

/** A record's export status at one destination. */
export const ExportStatusChip = ({
  status,
  size = 'small',
}: {
  status: ExportStatus;
  size?: 'small' | 'medium';
}) => {
  const { t } = useTranslation();
  const chip = statusChips[status];
  return (
    <Chip
      color={chip.color}
      icon={<chip.icon />}
      label={t(`extensionExports.status.${status}`)}
      size={size}
      variant="outlined"
    />
  );
};

/** Shown where a status chip would be when the status is unknown. */
export const ExportStatusUnknownIcon = CircleAlertIcon;
