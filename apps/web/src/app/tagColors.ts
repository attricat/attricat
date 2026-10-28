import colors from '../../design/tokens/colors.json';
import type { DesignMode } from './theme';

/** Preset colors a workspace can give a tag, as the API stores them. */
export const tagColorCodes = [
  'gray',
  'blue',
  'teal',
  'green',
  'yellow',
  'orange',
  'red',
  'purple',
  'pink',
] as const;
export type TagColor = (typeof tagColorCodes)[number];

/** A tag chip's fill and its text, which keeps 4.5:1 contrast on the fill. */
export type TagColorSwatch = { background: string; text: string };

const { palette, light, dark } = colors;

/**
 * Tag colors come from the design tokens where the system has a matching
 * tint and text pair. The design system has no categorical palette, so the
 * remaining values extend it for tags only and should move upstream:
 * purple and pink in both modes, the dark orange fill, and darker light-mode
 * orange and green text, since the token pairs fall short of 4.5:1 at chip
 * size.
 */
export const tagColorSwatches: Record<
  DesignMode,
  Record<TagColor, TagColorSwatch>
> = {
  light: {
    gray: { background: palette.slate[100], text: palette.slate[600] },
    blue: { background: palette.azure[100], text: palette.azure[900] },
    teal: {
      background: light.semantic.info.tint,
      text: light.semantic.info.main,
    },
    green: { background: light.semantic.success.tint, text: '#17703F' },
    yellow: {
      background: light.semantic.warning.tint,
      text: light.semantic.warning.foreground,
    },
    orange: { background: palette.copper[100], text: '#A1471B' },
    red: {
      background: light.semantic.error.tint,
      text: light.semantic.error.main,
    },
    purple: { background: '#F1EAFD', text: '#6B3FB8' },
    pink: { background: '#FCE8F3', text: '#A8326E' },
  },
  dark: {
    gray: { background: palette.slate[700], text: palette.slate[300] },
    blue: { background: dark.action.selected, text: palette.azure[300] },
    teal: {
      background: dark.semantic.info.tint,
      text: dark.semantic.info.main,
    },
    green: {
      background: dark.semantic.success.tint,
      text: dark.semantic.success.main,
    },
    yellow: {
      background: dark.semantic.warning.tint,
      text: dark.semantic.warning.foreground,
    },
    orange: { background: '#3A2A22', text: palette.copper[300] },
    red: {
      background: dark.semantic.error.tint,
      text: dark.semantic.error.main,
    },
    purple: { background: '#2C2350', text: '#C4A8FF' },
    pink: { background: '#3A1F35', text: '#F7A3CF' },
  },
};
