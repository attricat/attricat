import { useQuery } from '@tanstack/react-query';
import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  formatInstant,
  formatRelativeInstant,
  formatUtcInstant,
  type InstantStyle,
  type InstantValue,
  isBrowserTimeZone,
  resolveTimeZone,
} from './instantFormat';
import { sessionOptions } from '../features/auth/queryOptions';

/**
 * The stored preference from the session the app shell already loads, or
 * `null` when unset. It never fetches on its own so rendering many timestamps
 * does not refetch the session.
 */
export const usePreferredTimeZone = () =>
  useQuery({
    ...sessionOptions(),
    enabled: false,
    select: (session) => session?.time_zone ?? null,
  }).data ?? null;

/** The effective zone: the user's preference, else the browser zone. */
export const useTimeZone = () => resolveTimeZone(usePreferredTimeZone());

/**
 * Formatting bound to the active language and effective zone, for string
 * contexts such as translations, tooltips, and accessible labels. Prefer
 * `<Timestamp>` in JSX so the exact UTC value is available on hover/focus.
 */
export const useInstantFormat = () => {
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  const timeZone = useTimeZone();
  const showTimeZone = !isBrowserTimeZone(timeZone);
  const format = useCallback(
    (value: InstantValue, style?: InstantStyle) =>
      formatInstant(value, { locale, timeZone, style, showTimeZone }),
    [locale, showTimeZone, timeZone],
  );
  /** Local rendering followed by the exact UTC value, for plain text. */
  const formatWithUtc = useCallback(
    (value: InstantValue, style?: InstantStyle) =>
      `${format(value, style)} (${formatUtcInstant(value)})`,
    [format],
  );
  /** The distance from now, e.g. `3 days ago`; it does not depend on zone. */
  const formatRelative = useCallback(
    (value: InstantValue) => formatRelativeInstant(value, locale),
    [locale],
  );
  return useMemo(
    () => ({ format, formatRelative, formatWithUtc, locale, timeZone }),
    [format, formatRelative, formatWithUtc, locale, timeZone],
  );
};
