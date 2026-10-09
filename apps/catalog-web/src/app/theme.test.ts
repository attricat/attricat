import { describe, expect, it } from 'vitest';
import colors from '../../design/tokens/colors.json';
import radius from '../../design/tokens/radius.json';
import { makeTheme } from './theme';

describe('catalog theme', () => {
  it.each(['light', 'dark'] as const)('uses the %s design tokens', (mode) => {
    const theme = makeTheme(mode);
    const tokens = colors[mode];

    expect(theme.palette.mode).toBe(mode);
    expect(theme.palette.primary.main).toBe(tokens.brand.primary);
    expect(theme.palette.primary.contrastText).toBe(tokens.brand.onPrimary);
    expect(theme.palette.background.default).toBe(tokens.background.default);
    expect(theme.palette.background.paper).toBe(tokens.background.surface);
    expect(theme.palette.text.secondary).toBe(tokens.text.secondary);
    expect(theme.shape.borderRadius).toBe(radius.control);
    expect(theme.spacing(1)).toBe('4px');
  });

  it('sets light navigation apart from the page and keeps dark mode tokens', () => {
    expect(makeTheme('light').palette.background.navigation).toEqual({
      panel: colors.palette.slate['100'],
      rail: colors.palette.slate['100'],
    });
    expect(makeTheme('dark').palette.background.navigation).toEqual({
      panel: colors.dark.background.surface,
      rail: colors.dark.background.default,
    });
  });
});
