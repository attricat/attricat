import { MenuItem, TextField } from '@mui/material';

/** One revision a comparison can show, as its select labels it. */
export type RevisionOption = { version: number; label: string };

/** Chooses one side of a comparison between two revisions. */
export const RevisionSelect = ({
  label,
  onChange,
  options,
  value,
}: {
  label: string;
  onChange: (version: number) => void;
  options: readonly RevisionOption[];
  value: number | undefined;
}) => (
  <TextField
    label={label}
    onChange={(event) => onChange(Number(event.target.value))}
    select
    value={value ?? ''}
  >
    {options.map((option) => (
      <MenuItem key={option.version} value={option.version}>
        {option.label}
      </MenuItem>
    ))}
  </TextField>
);
