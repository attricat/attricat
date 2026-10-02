import { Stack } from '@mui/material';
import { Fragment, type ReactNode } from 'react';
import { NotSetValue } from '../components/values/NotSetValue';

const isMissing = (value: unknown) =>
  value === null ||
  value === undefined ||
  (typeof value === 'string' && !value.trim()) ||
  (Array.isArray(value) && value.length === 0);

/**
 * Renders a value with `renderItem`, or each value of a multi-valued table
 * projection. Absent values, including empty lists, render "Not set".
 */
export const EachValue = ({
  value,
  renderItem,
  direction = 'column',
}: {
  value: unknown;
  renderItem: (value: unknown) => ReactNode;
  direction?: 'row' | 'column';
}) => {
  const render = (item: unknown) =>
    isMissing(item) ? <NotSetValue /> : renderItem(item);
  if (!Array.isArray(value) || !value.length) return render(value);
  return (
    <Stack
      direction={direction}
      spacing={direction === 'row' ? 1 : 0.5}
      sx={{ flexWrap: 'wrap' }}
    >
      {value.map((item, index) => (
        <Fragment key={index}>
          {render(Array.isArray(item) ? JSON.stringify(item) : item)}
        </Fragment>
      ))}
    </Stack>
  );
};
