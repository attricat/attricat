import { Box, Chip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { tagChipSx, tagColorOf } from '../../tag-colors/tagColors';
import { tagColorsOptions } from '../../tag-colors/queryOptions';
import { systemTagLabel, visibleSystemTags } from '../systemTags';

type Props = {
  tags: readonly string[];
  /** Makes each chip a button, such as one that filters by its tag. */
  onSelect?: (tag: string) => void;
  /** The accessible name and tooltip of a chip that `onSelect` handles. */
  selectLabel?: (label: string) => string;
};

/**
 * A record's system tags as chips, in their stored order and workspace
 * colors. Tags render uncolored until the colors load, or if they fail to.
 */
export const SystemTagChips = ({ tags, onSelect, selectLabel }: Props) => {
  const colors = useQuery(tagColorsOptions());
  return (
    <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, minWidth: 0 }}>
      {visibleSystemTags(tags).map((tag) => {
        const label = systemTagLabel(tag);
        const color = tagColorOf(colors.data, tag);
        return (
          <Chip
            key={tag}
            label={label}
            {...(onSelect && {
              'aria-label': selectLabel?.(label),
              onClick: () => onSelect(tag),
            })}
            size="small"
            // Long tags are truncated; the title shows the whole tag.
            sx={[{ maxWidth: '100%' }, ...(color ? [tagChipSx(color)] : [])]}
            title={(onSelect && selectLabel?.(label)) || label}
            variant="outlined"
          />
        );
      })}
    </Box>
  );
};
