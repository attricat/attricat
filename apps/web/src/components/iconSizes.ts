// Lucide icons default to 1em of a 1.5rem font size, matching MUI's 24px
// standalone icons. Use these sizes for icons inside dense controls.
export const compactIconSize = 18;
export const smallIconSize = 20;
/** MUI's padding on each side of a small `IconButton`. */
const smallIconButtonPadding = 5;
/** The square a small `IconButton` around a compact icon occupies. */
export const compactIconButtonSize =
  compactIconSize + 2 * smallIconButtonPadding;
// Leads caption-sized text, such as a field's helper text.
export const captionIconSize = 14;
/** Spinners that sit beside text or take an icon's place in a control. */
export const progressSizes = {
  compact: 16,
  standard: 20,
} as const;
