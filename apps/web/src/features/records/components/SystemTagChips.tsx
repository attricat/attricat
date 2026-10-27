import { Box, Chip } from '@mui/material';
import { visibleSystemTags } from '../systemTags';

type Props = {
  tags: readonly string[];
  /** Makes each chip a button, such as one that filters by its tag. */
  onSelect?: (tag: string) => void;
  /** The accessible name and tooltip of a chip that `onSelect` handles. */
  selectLabel?: (tag: string) => string;
};

/** A record's system tags as chips, in their stored order. */
export const SystemTagChips = ({ tags, onSelect, selectLabel }: Props) => (
  <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, minWidth: 0 }}>
    {visibleSystemTags(tags).map((tag) => (
      <Chip
        key={tag}
        label={tag}
        {...(onSelect && {
          'aria-label': selectLabel?.(tag),
          onClick: () => onSelect(tag),
        })}
        size="small"
        // Long tags are truncated; the title shows the whole tag.
        sx={{ maxWidth: '100%' }}
        title={(onSelect && selectLabel?.(tag)) || tag}
        variant="outlined"
      />
    ))}
  </Box>
);
