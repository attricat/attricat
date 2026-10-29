import { FormControl, InputLabel, MenuItem, Select } from '@mui/material';
import { useId } from 'react';
import { ANY_FINDING_FILTER, FINDING_FILTER_MIN_WIDTH } from './constants';

import type { FilterOption } from './ruleFilterOptions';

/**
 * One filter of the findings or run history list; the empty value matches
 * everything.
 */
export const RuleListFilter = ({
  label,
  value,
  options,
  anyLabel,
  onChange,
}: {
  label: string;
  value: string | undefined;
  options: readonly FilterOption[];
  /** The label of the unset value, when the filter can be unset. */
  anyLabel?: string;
  onChange: (value: string | undefined) => void;
}) => {
  const labelId = useId();
  return (
    <FormControl sx={{ minWidth: FINDING_FILTER_MIN_WIDTH }}>
      <InputLabel id={labelId} shrink>
        {label}
      </InputLabel>
      <Select
        displayEmpty
        label={label}
        labelId={labelId}
        notched
        onChange={(event) =>
          onChange(
            event.target.value === ANY_FINDING_FILTER
              ? undefined
              : event.target.value,
          )
        }
        value={value ?? ANY_FINDING_FILTER}
      >
        {anyLabel !== undefined && (
          <MenuItem value={ANY_FINDING_FILTER}>{anyLabel}</MenuItem>
        )}
        {options.map((option) => (
          <MenuItem key={option.value} value={option.value}>
            {option.label}
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
};
