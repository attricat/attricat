import { Chip } from '@mui/material';
import { CircleXIcon } from 'lucide-react';
import type { Ref } from 'react';

type Props = {
  chipRef: Ref<HTMLDivElement>;
  label: string;
  onClick: () => void;
  onDelete: () => void;
  /** Accessible name of the delete icon, which the chip's Delete key also triggers. */
  removeLabel: string;
};

/** A filter in the Explorer filter bar: click to edit it, delete to remove it. */
export const ExplorerFilterChip = ({
  chipRef,
  label,
  onClick,
  onDelete,
  removeLabel,
}: Props) => (
  <Chip
    deleteIcon={<CircleXIcon aria-label={removeLabel} />}
    label={label}
    onClick={onClick}
    onDelete={onDelete}
    ref={chipRef}
    size="small"
    // Long values are truncated; the title shows the whole filter.
    sx={{ maxWidth: '100%', '& .MuiChip-label': { overflow: 'hidden' } }}
    title={label}
  />
);
