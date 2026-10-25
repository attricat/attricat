import { Chip, type ChipProps } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';

/** The colour and leading icon of one status in a feature's status map. */
export type StatusChipStyle = {
  color: NonNullable<ChipProps['color']>;
  icon: LucideIcon;
};

/** An outlined chip naming a status, coloured and iconed by its style. */
export const StatusChip = ({
  label,
  size,
  style: { color, icon: Icon },
}: {
  label: ReactNode;
  size?: ChipProps['size'];
  style: StatusChipStyle;
}) => (
  <Chip
    color={color}
    icon={<Icon />}
    label={label}
    size={size}
    variant="outlined"
  />
);
