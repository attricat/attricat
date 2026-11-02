import { InputAdornment, TextField, type TextFieldProps } from '@mui/material';
import { SearchIcon } from 'lucide-react';
import { smallIconSize } from './iconSizes';

/** A text field that narrows or searches a list, led by the search icon. */
export const SearchField = ({ slotProps, ...props }: TextFieldProps) => (
  <TextField
    {...props}
    slotProps={{
      ...slotProps,
      input: {
        startAdornment: (
          <InputAdornment position="start">
            <SearchIcon aria-hidden size={smallIconSize} />
          </InputAdornment>
        ),
      },
    }}
  />
);
