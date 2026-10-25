import {
  CircleCheckIcon,
  CircleHelpIcon,
  CircleXIcon,
  ClockIcon,
  RotateCcwIcon,
  UndoIcon,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ExportStatus } from './constants';
import { StatusChip, type StatusChipStyle } from '../../components/StatusChip';

const statusChips = {
  pending: { color: 'default', icon: ClockIcon },
  retrying: { color: 'warning', icon: RotateCcwIcon },
  uncertain: { color: 'warning', icon: CircleHelpIcon },
  rejected: { color: 'error', icon: CircleXIcon },
  synced: { color: 'success', icon: CircleCheckIcon },
  withdrawn: { color: 'default', icon: UndoIcon },
} as const satisfies Record<ExportStatus, StatusChipStyle>;

/** A record's export status at one destination. */
export const ExportStatusChip = ({
  status,
  size = 'small',
}: {
  status: ExportStatus;
  size?: 'small' | 'medium';
}) => {
  const { t } = useTranslation();
  return (
    <StatusChip
      label={t(`extensionExports.status.${status}`)}
      size={size}
      style={statusChips[status]}
    />
  );
};
