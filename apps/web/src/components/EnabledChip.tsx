import { Chip, type ChipProps } from '@mui/material';
import type { ReactNode } from 'react';

/**
 * Whether something runs, green while it does. The caller words the label so
 * that it agrees with what it describes.
 */
export const EnabledChip = ({
  enabled,
  label,
  size,
}: {
  enabled: boolean;
  label: ReactNode;
  size?: ChipProps['size'];
}) => (
  <Chip color={enabled ? 'success' : 'default'} label={label} size={size} />
);
