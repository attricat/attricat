import MoreHorizIcon from '@mui/icons-material/MoreHoriz';
import { IconButton, Popover, Stack, Tooltip } from '@mui/material';
import { useState } from 'react';
import type { ExtensionContribution } from './api';
import { OutletContribution } from './OutletContribution';
import { contributionKey, type OutletContext } from './outletContributions';

type ExtensionActionOverflowProps = {
  context?: OutletContext;
  contributions: ExtensionContribution[];
  label: string;
  onNavigate?: () => void;
};

export const ExtensionActionOverflow = ({
  context,
  contributions,
  label,
  onNavigate,
}: ExtensionActionOverflowProps) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <Tooltip title={label}>
        <IconButton
          aria-label={label}
          onClick={(event) => setAnchor(event.currentTarget)}
          size="small"
        >
          <MoreHorizIcon fontSize="inherit" />
        </IconButton>
      </Tooltip>
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
      >
        <Stack spacing={1} sx={{ p: 1 }}>
          {contributions.map((contribution) => (
            <OutletContribution
              context={context}
              contribution={contribution}
              key={contributionKey(contribution)}
              onNavigate={onNavigate}
            />
          ))}
        </Stack>
      </Popover>
    </>
  );
};
