import { visuallyHiddenSx } from '../../../components/VisuallyHidden';

/**
 * Fits a form control into a table cell: the column header names it, so its
 * label is kept only for assistive technology, and hints give way to errors.
 */
export const cellEditorSx = {
  minWidth: 120,
  // A list keeps room for its chosen label beside the open button.
  '&:has(.MuiAutocomplete-root)': { minWidth: 200 },
  position: 'relative',
  '& .MuiFormLabel-root': visuallyHiddenSx,
  '& .MuiOutlinedInput-notchedOutline': { top: 0 },
  '& .MuiOutlinedInput-notchedOutline legend': { display: 'none' },
  '& .MuiInputBase-input': { py: 0.75 },
  // A list's own padding would make it taller than the other inputs.
  '& .MuiAutocomplete-inputRoot.MuiOutlinedInput-root': { py: 0 },
  '& .MuiFormHelperText-root:not(.Mui-error)': visuallyHiddenSx,
} as const;

/** The row's action buttons: as narrow as they are, on one line. */
export const rowActionsCellSx = { whiteSpace: 'nowrap', width: '1%' } as const;
