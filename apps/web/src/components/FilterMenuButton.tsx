import { Button, Menu } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { compactOutlinedActionButtonSx } from './CompactOutlinedActionButton';
import { smallIconSize } from './iconSizes';

/** A compact button that offers a list filter's choices in a menu. */
export const FilterMenuButton = ({
  children,
  icon: Icon,
  label,
}: {
  /** The menu's items; call `close` when one is chosen. */
  children: (close: () => void) => ReactNode;
  icon: LucideIcon;
  label: ReactNode;
}) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const close = () => setAnchor(null);
  return (
    <>
      <Button
        color="primary"
        onClick={(event) => setAnchor(event.currentTarget)}
        size="small"
        startIcon={<Icon size={smallIconSize} />}
        sx={compactOutlinedActionButtonSx}
        variant="outlined"
      >
        {label}
      </Button>
      <Menu anchorEl={anchor} onClose={close} open={Boolean(anchor)}>
        {children(close)}
      </Menu>
    </>
  );
};
