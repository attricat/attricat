import { Box } from '@mui/material';
import type { TagColor } from '../../app/tagColors';
import { tagColorDotSize } from './constants';

/** A small disc in a tag color, beside a tag or color name. Decorative. */
export const TagColorDot = ({ color }: { color: TagColor }) => (
  <Box
    aria-hidden
    component="span"
    sx={(theme) => ({
      // The text shade, which stays vivid where the chip fill is faint.
      backgroundColor: theme.palette.tagColors[color].text,
      borderRadius: '50%',
      display: 'inline-block',
      flexShrink: 0,
      height: tagColorDotSize,
      width: tagColorDotSize,
    })}
  />
);
