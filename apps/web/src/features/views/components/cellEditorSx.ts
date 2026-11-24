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
  '& .MuiFormHelperText-root:not(.Mui-error)': visuallyHiddenSx,
} as const;
