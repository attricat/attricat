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
import colors from '../../design/tokens/colors.json';
import typography from '../../design/tokens/typography.json';
import radius from '../../design/tokens/radius.json';
import elevation from '../../design/tokens/elevation.json';

// These tokens are a pinned snapshot from attricat/design; see design/README.md.
// These overrides adapt its MUI patterns to the version used by catalog-web.
export type DesignMode = 'light' | 'dark';

declare module '@mui/material/styles' {
  interface TypeBackground {
    /** Icon rail and its flyout panels. */
    navigation: { panel: string; rail: string };
  }
  interface TypeAction {
    /** Hover on navigation items, which sit on the navigation background. */
    navigationHover: string;
  }
}

export const monoFontFamily = typography.families.mono;

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
  const border = `1px solid ${c.border.default}`;

  return createTheme({
    palette: {
      mode,
      primary: { main: c.brand.primary, contrastText: c.brand.onPrimary },
      secondary: { main: c.brand.accent },
      success: { main: c.semantic.success.main },
      warning: {
        main: c.semantic.warning.main,
        contrastText: c.semantic.warning.onMain,
      },
      error: { main: c.semantic.error.main },
      info: { main: c.semantic.info.main },
      background: {
        default: c.background.default,
        paper: c.background.surface,
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
        selected: c.action.selected,
        disabledBackground: c.action.disabled,
        focus: c.action.hover,
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
            textTransform: 'none',
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
      MuiTextField: { defaultProps: { size: 'small', variant: 'outlined' } },
      MuiFormControl: { defaultProps: { size: 'small' } },
      MuiSelect: {
        defaultProps: { IconComponent: ChevronDownIcon, size: 'small' },
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
          input: { fontSize: 14 },
        },
      },
      MuiInputLabel: { styleOverrides: { root: { fontSize: 14 } } },
      MuiCheckbox: {
        defaultProps: { size: 'small' },
        styleOverrides: { root: { padding: 6 } },
      },
      MuiSwitch: { defaultProps: { size: 'small' } },
      MuiChip: {
        defaultProps: { deleteIcon: createElement(CircleXIcon), size: 'small' },
        styleOverrides: {
          root: { borderRadius: radius.small, fontSize: 12, fontWeight: 600 },
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
          root: {
            borderColor: c.border.default,
            borderRadius: radius.surface,
            backgroundImage: 'none',
          },
        },
      },
      MuiPaper: {
        styleOverrides: {
          root: { backgroundImage: 'none' },
          outlined: { borderColor: c.border.default },
        },
      },
      MuiDialog: {
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
            fontSize: 12,
          },
          arrow: { color: c.background.elevated },
        },
      },
      MuiTabs: {
        styleOverrides: {
          root: { minHeight: 40, borderBottom: `1px solid ${c.border.subtle}` },
          indicator: { height: 2 },
        },
      },
      MuiTab: {
        defaultProps: { iconPosition: 'start' },
        styleOverrides: {
          root: {
            textTransform: 'none',
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
            backgroundImage: 'none',
            backgroundColor: c.background.surface,
            borderBottom: `1px solid ${c.border.subtle}`,
          },
        },
      },
      MuiDrawer: {
        styleOverrides: {
          paper: {
            backgroundColor: c.background.surface,
            backgroundImage: 'none',
            borderRight: `1px solid ${c.border.subtle}`,
          },
        },
      },
      MuiTable: { defaultProps: { size: 'small' } },
      MuiTableSortLabel: { defaultProps: { IconComponent: ArrowDownIcon } },
      MuiTableCell: {
        styleOverrides: {
          root: {
            borderBottom: `1px solid ${c.border.subtle}`,
            padding: '10px 16px',
          },
          head: {
            color: c.text.secondary,
            fontSize: 12,
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
        styleOverrides: { root: { fontSize: 14, minHeight: 36 } },
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
