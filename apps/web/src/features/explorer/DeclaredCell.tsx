import { Typography } from '@mui/material';
import { useMemo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  maximumCellPrecision,
  maximumCellUnitLength,
} from '../extensions/constants';
import type { CellDeclaration } from '../extensions/schemas';

type Setting = { prop: string } | number | string | undefined;

/** A display setting's literal, or the column's value for the prop it names. */
const resolveSetting = (setting: Setting, props: Record<string, unknown>) =>
  typeof setting === 'object' ? props[setting.prop] : setting;

const precisionOf = (value: unknown) =>
  typeof value === 'number' &&
  Number.isInteger(value) &&
  value >= 0 &&
  value <= maximumCellPrecision
    ? value
    : undefined;

const unitOf = (value: unknown) =>
  typeof value === 'string' &&
  value.trim() !== '' &&
  value.length <= maximumCellUnitLength
    ? value
    : undefined;

/**
 * A table cell an extension declares. The host draws each value as the
 * declaration says, so no extension code runs per cell; a value the display
 * cannot show keeps the host's own rendering.
 */
export const DeclaredCell = ({
  cell,
  fallback,
  props,
  value,
}: {
  cell: CellDeclaration;
  fallback: ReactNode;
  /** The props the table column passes. */
  props: Record<string, unknown>;
  value: unknown;
}) => {
  const { i18n } = useTranslation();
  const { display } = cell;
  const precision = precisionOf(resolveSetting(display.precision, props));
  const unit = unitOf(resolveSetting(display.unit, props));
  const format = useMemo(
    () =>
      new Intl.NumberFormat(
        i18n.language,
        precision === undefined
          ? undefined
          : {
              maximumFractionDigits: precision,
              minimumFractionDigits: precision,
            },
      ),
    [i18n.language, precision],
  );
  // A relationship path to several records has a value for each.
  const values = Array.isArray(value) ? value : [value];
  if (
    values.length === 0 ||
    !values.every((item) => typeof item === 'number' && Number.isFinite(item))
  )
    return fallback;
  const text = (values as number[])
    .map((item) => format.format(item))
    .join(', ');
  return (
    <Typography
      noWrap
      sx={{ fontVariantNumeric: 'tabular-nums', textAlign: 'right' }}
      variant="body2"
    >
      {unit ? `${text} ${unit}` : text}
    </Typography>
  );
};
