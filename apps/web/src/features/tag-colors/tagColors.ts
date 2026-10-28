import { alpha, type Theme } from '@mui/material/styles';
import type { TagColor } from '../../app/tagColors';
import type { TagColors } from './schemas';

const own = (colors: Record<string, TagColor>, tag: string) =>
  Object.hasOwn(colors, tag) ? colors[tag] : undefined;

/** A tag's color: the workspace's choice, else its default, else none. */
export const tagColorOf = (
  colors: TagColors | undefined,
  tag: string,
): TagColor | undefined =>
  colors && (own(colors.overrides, tag) ?? own(colors.defaults, tag));

/**
 * A colored tag chip: its fill, its text, and a border in the text color so
 * the chip keeps its edge on a matching surface. A clickable chip darkens on
 * hover and focus; a removable chip's delete icon takes the text color.
 */
export const tagChipSx = (color: TagColor) => (theme: Theme) => {
  const { background, text } = theme.palette.tagColors[color];
  const shade = alpha(text, 0.12);
  return {
    backgroundColor: background,
    borderColor: alpha(text, 0.4),
    color: text,
    '& .MuiChip-deleteIcon': { color: alpha(text, 0.7) },
    '& .MuiChip-deleteIcon:hover': { color: text },
    '&.MuiChip-clickable:hover, &.MuiChip-clickable.Mui-focusVisible': {
      backgroundColor: background,
      backgroundImage: `linear-gradient(${shade}, ${shade})`,
    },
  };
};
