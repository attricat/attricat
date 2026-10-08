import { Box } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { compactIconSize } from './iconSizes';

type Props = {
  children: ReactNode;
  icon: LucideIcon;
};

/**
 * Text led by a decorative icon. Use it for select options, whose content is
 * also rendered as the selected value, and other inline labels.
 */
export const IconLabel = ({ children, icon: Icon }: Props) => (
  <Box
    component="span"
    sx={{ alignItems: 'center', display: 'inline-flex', gap: 3 }}
  >
    <Box
      component="span"
      sx={{ color: 'text.secondary', display: 'inline-flex', flexShrink: 0 }}
    >
      <Icon aria-hidden size={compactIconSize} />
    </Box>
    {children}
  </Box>
);
