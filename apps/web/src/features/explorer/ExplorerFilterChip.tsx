import { Chip } from '@mui/material';
import { CircleXIcon } from 'lucide-react';
import type { Ref } from 'react';
import type { TagColor } from '../../app/tagColors';
import { tagChipSx } from '../tag-colors/tagColors';

type Props = {
  /** The workspace color of the tag a tag filter requires. */
  color?: TagColor;
  chipRef: Ref<HTMLDivElement>;
  label: string;
  /** Opens the filter's editor; filters without one are only removed. */
  onClick?: () => void;
  onDelete: () => void;
  /** Accessible name of the delete icon, which the chip's Delete key also triggers. */
  removeLabel: string;
};

/** A filter in the Explorer filter bar: click to edit it, delete to remove it. */
export const ExplorerFilterChip = ({
  color,
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
    sx={[
      { maxWidth: '100%', '& .MuiChip-label': { overflow: 'hidden' } },
      ...(color ? [tagChipSx(color)] : []),
    ]}
    title={label}
  />
);
