import { useTheme } from '@mui/material/styles';
import colors from '../../design/tokens/colors.json';
import type { DesignMode } from '../app/theme';

/** Monaco theme names, one per design mode. */
export const monacoThemeNames: Record<DesignMode, string> = {
  light: 'attricat-light',
  dark: 'attricat-dark',
};

/** Monaco's built-in themes that supply syntax colors for each mode. */
export const monacoBaseThemes = { light: 'vs', dark: 'vs-dark' } as const;

/** Editor chrome colors from the design tokens, keyed by Monaco color id. */
export const monacoThemeColors = (mode: DesignMode) => {
  const c = colors[mode];
  return {
    'editor.background': c.background.surface,
    'editor.foreground': c.text.primary,
    'editor.lineHighlightBackground': c.action.hover,
    'editor.lineHighlightBorder': c.action.hover,
    'editor.selectionBackground': c.action.selected,
    'editor.inactiveSelectionBackground': c.action.selected,
    'editorCursor.foreground': c.brand.primary,
    'editorLineNumber.foreground': c.text.disabled,
    'editorLineNumber.activeForeground': c.text.secondary,
    'editorIndentGuide.background1': c.border.subtle,
    'editorIndentGuide.activeBackground1': c.border.default,
    'editorWidget.background': c.background.elevated,
    'editorWidget.border': c.border.default,
    'editorSuggestWidget.background': c.background.elevated,
    'editorSuggestWidget.border': c.border.default,
    'editorSuggestWidget.selectedBackground': c.action.selected,
    'editorHoverWidget.background': c.background.elevated,
    'editorHoverWidget.border': c.border.default,
    focusBorder: c.action.focus,
  };
};

/** The Monaco theme that matches the app's current color mode. */
export const useMonacoTheme = () => monacoThemeNames[useTheme().palette.mode];
