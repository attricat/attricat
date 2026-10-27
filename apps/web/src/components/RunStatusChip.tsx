import {
  BanIcon,
  CircleCheckIcon,
  CircleXIcon,
  ClockIcon,
  LoaderIcon,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { RunStatus } from '../api/runStatus';
import { StatusChip, type StatusChipStyle } from './StatusChip';

const statusChips = {
  pending: { color: 'default', icon: ClockIcon },
  leased: { color: 'info', icon: LoaderIcon },
  completed: { color: 'success', icon: CircleCheckIcon },
  dead_letter: { color: 'error', icon: CircleXIcon },
  cancelled: { color: 'default', icon: BanIcon },
} as const satisfies Record<RunStatus, StatusChipStyle>;

/** The status of a rule or workflow run. */
export const RunStatusChip = ({ status }: { status: RunStatus }) => {
  const { t } = useTranslation();
  return (
    <StatusChip
      label={t(`common.runStatuses.${status}`)}
      size="small"
      style={statusChips[status]}
    />
  );
};
