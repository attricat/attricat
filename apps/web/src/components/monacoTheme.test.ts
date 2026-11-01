import { getContrastRatio } from '@mui/material/styles';
import { describe, expect, it } from 'vitest';
import { monacoThemeColors, monacoThemeRules } from './monacoTheme';

/** WCAG AA contrast for editor-sized text. */
const minimumTextContrast = 4.5;

describe('Monaco theme', () => {
  it.each(['light', 'dark'] as const)(
    'keeps %s syntax overrides readable on the editor and its highlighted line',
    (mode) => {
      const editor = monacoThemeColors(mode);
      for (const rule of monacoThemeRules(mode))
        for (const background of [
          editor['editor.background'],
          editor['editor.lineHighlightBackground'],
        ])
          expect(
            getContrastRatio(`#${rule.foreground}`, background),
            `${rule.token} on ${background}`,
          ).toBeGreaterThanOrEqual(minimumTextContrast);
    },
  );
});
