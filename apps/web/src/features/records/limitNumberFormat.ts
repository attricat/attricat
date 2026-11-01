import i18n from 'i18next';

/**
 * Formats the limits that hints and validation messages name. Four-digit
 * numbers stay ungrouped, so a year limit reads 1990 rather than 1,990.
 */
export const limitNumberFormat = () =>
  new Intl.NumberFormat(i18n.language, { useGrouping: 'min2' });
