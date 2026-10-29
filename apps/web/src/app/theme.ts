import { createTheme, type Shadows } from '@mui/material/styles';
import {
  ArrowDownIcon,
  ChevronDownIcon,
  CircleAlertIcon,
  CircleCheckIcon,
  CircleXIcon,
  InfoIcon,
  TriangleAlertIcon,
  type LucideIcon,
} from 'lucide-react';
import { createElement } from 'react';
import { smallIconSize } from '../components/iconSizes';
import { focusFirstField } from '../components/initialFocus';
import colors from '../../design/tokens/colors.json';
import typography from '../../design/tokens/typography.json';
import radius from '../../design/tokens/radius.json';
import elevation from '../../design/tokens/elevation.json';
import {
  tagColorSwatches,
  type TagColor,
  type TagColorSwatch,
} from './tagColors';

// These tokens are a pinned snapshot from attricat/design; see design/README.md.
// These overrides adapt its MUI patterns to the version used by web.
export type DesignMode = 'light' | 'dark';

declare module '@mui/material/styles' {
  interface TypeBackground {
    /** Icon rail and its flyout panels. */
    navigation: { panel: string; rail: string };
    /** Regions nested inside a surface, such as a code block. */
    inset: string;
  }
  interface TypeAction {
    /** Hover on navigation items, which sit on the navigation background. */
    navigationHover: string;
    /** Marks a field changed before an explicit Save. */
    changed: string;
    /** Outlines the element that has keyboard focus. */
    focusRing: string;
  }
  interface PaletteColor {
    /** A faint background in the color, behind content that keeps its own. */
    tint?: string;
  }
  interface SimplePaletteColorOptions {
    tint?: string;
  }
  interface Palette {
    /** Fill and text of each preset tag color. */
    tagColors: Record<TagColor, TagColorSwatch>;
  }
  interface PaletteOptions {
    tagColors?: Record<TagColor, TagColorSwatch>;
  }
}

export const monoFontFamily = typography.families.mono;

/** Keyboard focus outline for custom focusable surfaces, as on IconButton. */
export const focusRing = {
  outline: '2px solid',
  outlineColor: 'action.focusRing',
  outlineOffset: 2,
} as const;

/** The design system's fully rounded radius, for pill-shaped controls. */
export const pillRadius = radius.pill;

/** The design system's code role: mono at caption size. */
export const codeTypography = {
  fontFamily: monoFontFamily,
  fontSize: typography.roles.code.size,
  lineHeight: typography.roles.code.lineHeight,
} as const;

// Icons rendered inside a component that sizes them with its own font size.
const inheritSizeIcon = (icon: LucideIcon) =>
  createElement(icon, { style: { fontSize: 'inherit' } });

export const makeTheme = (mode: DesignMode) => {
  const c = colors[mode];
  const role = typography.roles;
  const type = (r: typeof role.body) => ({
    fontSize: r.size,
    fontWeight: r.weight,
    lineHeight: r.lineHeight,
    letterSpacing: r.tracking,
  });
  // A floated field label shrinks from body to label size.
  const shrunkLabelScale = role.label.size / role.body.size;
  const border = `1px solid ${c.border.default}`;
  const subtleBorder = `1px solid ${c.border.subtle}`;

  return createTheme({
    palette: {
      mode,
      primary: { main: c.brand.primary, contrastText: c.brand.onPrimary },
      secondary: { main: c.brand.accent },
      success: {
        main: c.semantic.success.main,
        tint: c.semantic.success.tint,
      },
      warning: {
        main: c.semantic.warning.main,
        contrastText: c.semantic.warning.onMain,
        tint: c.semantic.warning.tint,
      },
      error: { main: c.semantic.error.main, tint: c.semantic.error.tint },
      info: { main: c.semantic.info.main, tint: c.semantic.info.tint },
      tagColors: tagColorSwatches[mode],
      background: {
        default: c.background.default,
        paper: c.background.surface,
        inset: c.background.inset,
        navigation: {
          panel: c.background.navigation,
          rail: c.background.navigationRail,
        },
      },
      text: {
        primary: c.text.primary,
        secondary: c.text.secondary,
        disabled: c.text.disabled,
      },
      divider: c.border.default,
      action: {
        hover: c.action.hover,
        navigationHover: c.action.navigationHover,
        changed: c.action.changed,
        selected: c.action.selected,
        disabledBackground: c.action.disabled,
        focus: c.action.hover,
        focusRing: c.action.focus,
      },
    },
    typography: {
      fontFamily: typography.families.sans,
      fontSize: 14,
      h1: type(role.display),
      h2: type(role.marketingHeading),
      h3: type(role.appHeading),
      h4: type(role.sectionHeading),
      h5: type(role.sectionHeading),
      h6: type(role.label),
      body1: type(role.body),
      body2: type(role.body),
      subtitle1: type(role.label),
      subtitle2: type(role.caption),
      caption: type(role.caption),
      overline: {
        ...type(role.caption),
        fontWeight: 600,
        textTransform: 'uppercase',
        letterSpacing: '0.09em',
      },
      button: { ...type(role.label), textTransform: 'none' },
    },
    spacing: 4,
    shape: { borderRadius: radius.control },
    shadows: Array.from({ length: 25 }, (_, index) =>
      index === 0
        ? 'none'
        : index < 5
          ? elevation[mode].raised
          : elevation[mode].overlay,
    ) as Shadows,
    components: {
      MuiCssBaseline: {
        styleOverrides: {
          body: {
            scrollbarColor: `${c.border.strong} ${c.background.default}`,
          },
          '::selection': { backgroundColor: c.action.selected },
          // LucideProvider sizes icons at 1em; match MUI's standalone icon size
          // while letting component styles such as button icons override it.
          ':where(.lucide)': { flexShrink: 0, fontSize: '1.5rem' },
          'a:not(.MuiButtonBase-root), a:not(.MuiButtonBase-root):visited': {
            color: c.brand.primary,
          },
        },
      },
      MuiButton: {
        defaultProps: { disableElevation: true, size: 'small' },
        styleOverrides: {
          root: {
            borderRadius: radius.control,
            fontWeight: 600,
            minHeight: 36,
            paddingInline: 14,
          },
          outlined: {
            borderColor: c.border.default,
            '&:hover': {
              borderColor: c.brand.primary,
              backgroundColor: c.action.hover,
            },
          },
        },
      },
      MuiIconButton: {
        defaultProps: { size: 'small' },
        styleOverrides: {
          root: {
            borderRadius: radius.control,
            '&:focus-visible': {
              outline: `2px solid ${c.action.focus}`,
              outlineOffset: 2,
            },
          },
        },
      },
      MuiTextField: { defaultProps: { variant: 'outlined' } },
      MuiSelect: {
        defaultProps: { IconComponent: ChevronDownIcon },
        styleOverrides: { icon: { fontSize: smallIconSize } },
      },
      MuiNativeSelect: {
        defaultProps: { IconComponent: ChevronDownIcon },
        styleOverrides: { icon: { fontSize: smallIconSize } },
      },
      MuiOutlinedInput: {
        styleOverrides: {
          root: {
            borderRadius: radius.control,
            backgroundColor: c.background.surface,
            '& .MuiOutlinedInput-notchedOutline': {
              borderColor: c.border.default,
            },
            '&:hover .MuiOutlinedInput-notchedOutline': {
              borderColor: c.border.strong,
            },
          },
          input: { fontSize: role.body.size },
          // The notch is sized by a hidden copy of the floated label.
          notchedOutline: { '& legend': { fontSize: `${shrunkLabelScale}em` } },
        },
      },
      MuiInputLabel: {
        styleOverrides: {
          root: { fontSize: role.body.size },
          shrink: {
            '&.MuiInputLabel-outlined': {
              transform: `translate(14px, -9px) scale(${shrunkLabelScale})`,
            },
          },
        },
      },
      MuiCheckbox: {
        defaultProps: { size: 'small' },
        styleOverrides: { root: { padding: 6 } },
      },
      MuiSwitch: { defaultProps: { size: 'small' } },
      MuiChip: {
        defaultProps: { deleteIcon: createElement(CircleXIcon), size: 'small' },
        styleOverrides: {
          root: {
            borderRadius: radius.small,
            fontSize: role.caption.size,
            fontWeight: 600,
          },
          outlined: { borderColor: c.border.default },
          colorWarning: {
            '&.MuiChip-outlined': {
              color: c.semantic.warning.foreground,
              backgroundColor: c.semantic.warning.tint,
              borderColor: c.semantic.warning.main,
            },
            '&.MuiChip-filled': { color: c.semantic.warning.onMain },
          },
        },
      },
      MuiAlert: {
        defaultProps: {
          iconMapping: {
            error: inheritSizeIcon(CircleAlertIcon),
            info: inheritSizeIcon(InfoIcon),
            success: inheritSizeIcon(CircleCheckIcon),
            warning: inheritSizeIcon(TriangleAlertIcon),
          },
        },
        styleOverrides: {
          root: {
            alignItems: 'center',
            border,
            borderRadius: radius.surface,
            '&.MuiAlert-standardWarning, &.MuiAlert-outlinedWarning': {
              color: c.semantic.warning.foreground,
              backgroundColor: c.semantic.warning.tint,
              '& .MuiAlert-icon': { color: c.semantic.warning.foreground },
            },
          },
        },
      },
      MuiCard: {
        defaultProps: { variant: 'outlined' },
        styleOverrides: {
          root: { borderRadius: radius.surface },
        },
      },
      MuiPaper: {
        styleOverrides: {
          root: { backgroundImage: 'none' },
          outlined: { borderColor: c.border.default },
        },
      },
      MuiDialog: {
        defaultProps: {
          // A dialog's own `onEntered` replaces this, to focus another field.
          slotProps: {
            transition: {
              onEntered: (node: HTMLElement) => focusFirstField(node),
            },
          },
        },
        styleOverrides: {
          paper: {
            border,
            borderRadius: radius.dialog,
            boxShadow: elevation[mode].overlay,
          },
        },
      },
      MuiTooltip: {
        defaultProps: { arrow: true },
        styleOverrides: {
          tooltip: {
            backgroundColor: c.background.elevated,
            color: c.text.primary,
            border,
            boxShadow: elevation[mode].raised,
            fontSize: role.caption.size,
          },
          arrow: { color: c.background.elevated },
        },
      },
      MuiTabs: {
        styleOverrides: {
          root: { minHeight: 40, borderBottom: subtleBorder },
          indicator: { height: 2 },
        },
      },
      MuiTab: {
        defaultProps: { iconPosition: 'start' },
        styleOverrides: {
          root: {
            minHeight: 40,
            minWidth: 0,
            padding: '8px 16px',
            fontWeight: 600,
          },
          // The 12px icon–label gap from the design system.
          labelIcon: { '& .MuiTab-icon': { marginRight: 12 } },
        },
      },
      MuiAppBar: {
        defaultProps: { elevation: 0, color: 'default' },
        styleOverrides: {
          root: {
            backgroundColor: c.background.surface,
            borderBottom: subtleBorder,
          },
        },
      },
      MuiDrawer: {
        styleOverrides: {
          paper: {
            backgroundColor: c.background.surface,
            borderRight: subtleBorder,
          },
        },
      },
      MuiTable: { defaultProps: { size: 'small' } },
      MuiTableSortLabel: { defaultProps: { IconComponent: ArrowDownIcon } },
      MuiTableCell: {
        styleOverrides: {
          root: {
            borderBottom: subtleBorder,
            padding: '10px 16px',
          },
          head: {
            color: c.text.secondary,
            fontSize: role.caption.size,
            fontWeight: 600,
            backgroundColor: c.background.inset,
            whiteSpace: 'nowrap',
          },
        },
      },
      MuiTableRow: {
        styleOverrides: {
          root: { '&:hover': { backgroundColor: c.action.hover } },
        },
      },
      MuiListItemIcon: {
        // A 24px icon plus the 12px icon–label gap from the design system.
        styleOverrides: { root: { minWidth: 36 } },
      },
      MuiMenu: {
        styleOverrides: {
          paper: { border, boxShadow: elevation[mode].overlay, marginTop: 4 },
        },
      },
      MuiMenuItem: {
        styleOverrides: { root: { fontSize: role.body.size, minHeight: 36 } },
      },
      MuiDivider: {
        styleOverrides: { root: { borderColor: c.border.subtle } },
      },
      MuiPaginationItem: {
        styleOverrides: { root: { borderRadius: radius.control } },
      },
    },
  });
};
