import { getContrastRatio } from '@mui/material/styles';
import { describe, expect, it } from 'vitest';
import colors from '../../design/tokens/colors.json';
import radius from '../../design/tokens/radius.json';
import { tagColorCodes } from './tagColors';
import { makeTheme } from './theme';

/** WCAG AA contrast for chip-sized text. */
const minimumTextContrast = 4.5;

describe('catalog theme', () => {
  it.each(['light', 'dark'] as const)('uses the %s design tokens', (mode) => {
    const theme = makeTheme(mode);
    const tokens = colors[mode];

    expect(theme.palette.mode).toBe(mode);
    expect(theme.palette.primary.main).toBe(tokens.brand.primary);
    expect(theme.palette.primary.contrastText).toBe(tokens.brand.onPrimary);
    expect(theme.palette.background.default).toBe(tokens.background.default);
    expect(theme.palette.background.paper).toBe(tokens.background.surface);
    expect(theme.palette.background.navigation).toEqual({
      panel: tokens.background.navigation,
      rail: tokens.background.navigationRail,
    });
    expect(theme.palette.action.navigationHover).toBe(
      tokens.action.navigationHover,
    );
    expect(theme.palette.action.focusRing).toBe(tokens.action.focus);
    expect(theme.palette.text.secondary).toBe(tokens.text.secondary);
    expect(theme.shape.borderRadius).toBe(radius.control);
    expect(theme.spacing(1)).toBe('4px');
  });

  it.each(['light', 'dark'] as const)(
    'keeps %s tag text readable on its fill',
    (mode) => {
      const { tagColors } = makeTheme(mode).palette;
      for (const color of tagColorCodes) {
        const { background, text } = tagColors[color];
        expect(
          getContrastRatio(text, background),
          color,
        ).toBeGreaterThanOrEqual(minimumTextContrast);
      }
    },
  );
});
