import { IconButton, Tooltip, type SxProps, type Theme } from '@mui/material';
import { CopyIcon } from 'lucide-react';
import { compactIconSize } from './iconSizes';
import { useCopyToClipboard } from './useCopyToClipboard';

/** Copies a value, which may be worked out only when clicked. */
export const CopyIconButton = ({
  label,
  sx,
  value,
}: {
  label: string;
  sx?: SxProps<Theme>;
  value: string | (() => string);
}) => {
  const copyText = useCopyToClipboard();
  return (
    <Tooltip title={label}>
      <IconButton
        aria-label={label}
        onClick={(event) => {
          // It can sit inside a row or panel that reacts to clicks.
          event.stopPropagation();
          void copyText(typeof value === 'function' ? value() : value);
        }}
        size="small"
        sx={sx}
      >
        <CopyIcon size={compactIconSize} />
      </IconButton>
    </Tooltip>
  );
};
