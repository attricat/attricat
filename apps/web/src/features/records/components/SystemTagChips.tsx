import { Box, Chip } from '@mui/material';
import { visibleSystemTags } from '../systemTags';

/** A record's system tags as read-only chips, in their stored order. */
export const SystemTagChips = ({ tags }: { tags: readonly string[] }) => (
  <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, minWidth: 0 }}>
    {visibleSystemTags(tags).map((tag) => (
      <Chip
        key={tag}
        label={tag}
        size="small"
        // Long tags are truncated; the title shows the whole tag.
        sx={{ maxWidth: '100%' }}
        title={tag}
        variant="outlined"
      />
    ))}
  </Box>
);
