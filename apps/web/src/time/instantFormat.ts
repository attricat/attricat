/**
 * The single entry point for rendering instants. The API stores and returns
 * UTC; the UI renders instants in the user's preferred zone. Components should
 * use `<Timestamp>` or `useInstantFormat`, which resolve locale and zone.
 */

/** Named presentation styles; add a style here rather than ad-hoc options. */
export const instantStyles = {
  dateTime: { dateStyle: 'medium', timeStyle: 'short' },
  dateTimeSeconds: { dateStyle: 'medium', timeStyle: 'medium' },
  date: { dateStyle: 'medium' },
  time: { timeStyle: 'medium' },
} as const satisfies Record<string, Intl.DateTimeFormatOptions>;

export type InstantStyle = keyof typeof instantStyles;

export type InstantValue = string | number | Date;

export type InstantFormatOptions = {
  locale: string;
  timeZone: string;
  style?: InstantStyle;
  /** Appends a short zone name, e.g. `CEST` or `GMT+2`. */
  showTimeZone?: boolean;
};

export const utcTimeZone = 'UTC';
const defaultInstantStyle: InstantStyle = 'dateTime';
const zoneLabelOptions = {
  hour: 'numeric',
  timeZoneName: 'short',
} as const satisfies Intl.DateTimeFormatOptions;
// `YYYY-MM-DDTHH:mm:ss` from an ISO string, before fractional seconds.
const utcSecondsLength = 19;
// `YYYY-MM-DDTHH:mm`, the value a `datetime-local` input edits.
const localDateTimeMinutesLength = 16;
const secondMilliseconds = 1000;
const dayMilliseconds = 86_400_000;

const formatters = new Map<string, Intl.DateTimeFormat>();

const formatter = (
  locale: string,
  timeZone: string,
  optionsKey: string,
  options: Intl.DateTimeFormatOptions,
) => {
  const key = `${locale}\u0000${timeZone}\u0000${optionsKey}`;
  let cached = formatters.get(key);
  if (!cached) {
    cached = new Intl.DateTimeFormat(locale, { ...options, timeZone });
    formatters.set(key, cached);
  }
  return cached;
};

/** Returns `undefined` for values that do not identify a real instant. */
export const parseInstant = (value: InstantValue) => {
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? undefined : date;
};

export const isValidTimeZone = (timeZone: string) => {
  if (!timeZone) return false;
  try {
    new Intl.DateTimeFormat(undefined, { timeZone });
    return true;
  } catch {
    return false;
  }
};

/** Canonical spelling so aliases such as `Etc/UTC` and `UTC` compare equal. */
export const canonicalTimeZone = (timeZone: string) =>
  formatter('en', timeZone, 'canonical', {}).resolvedOptions().timeZone;

export const browserTimeZone = () =>
  new Intl.DateTimeFormat().resolvedOptions().timeZone;

/** The preferred zone when the runtime knows it, otherwise the browser's. */
export const resolveTimeZone = (preferred: string | null | undefined) =>
  preferred && isValidTimeZone(preferred) ? preferred : browserTimeZone();

export const isBrowserTimeZone = (timeZone: string) =>
  canonicalTimeZone(timeZone) === canonicalTimeZone(browserTimeZone());

/** IANA zones the runtime supports, always including `UTC`. */
export const supportedTimeZones = (): readonly string[] => {
  const zones = Intl.supportedValuesOf('timeZone');
  return zones.includes(utcTimeZone) ? zones : [utcTimeZone, ...zones];
};

export const timeZoneLabel = (
  value: InstantValue,
  locale: string,
  timeZone: string,
) => {
  const date = parseInstant(value);
  if (!date) return '';
  return (
    formatter(locale, timeZone, 'zoneLabel', zoneLabelOptions)
      .formatToParts(date)
      .find((part) => part.type === 'timeZoneName')?.value ?? timeZone
  );
};

/**
 * Formats an instant in `timeZone`. Values that are not instants are returned
 * unchanged so malformed data stays visible rather than throwing.
 */
export const formatInstant = (
  value: InstantValue,
  {
    locale,
    timeZone,
    style = defaultInstantStyle,
    showTimeZone = false,
  }: InstantFormatOptions,
) => {
  const date = parseInstant(value);
  if (!date) return String(value);
  const formatted = formatter(
    locale,
    timeZone,
    style,
    instantStyles[style],
  ).format(date);
  return showTimeZone
    ? `${formatted} ${timeZoneLabel(date, locale, timeZone)}`
    : formatted;
};

/**
 * Formats a calendar date (`YYYY-MM-DD`). Calendar dates are not instants, so
 * they are never shifted into the user's zone.
 */
export const formatCalendarDate = (value: string, locale: string) =>
  formatInstant(`${value}T00:00:00Z`, {
    locale,
    timeZone: utcTimeZone,
    style: 'date',
  });

/** Locale-independent exact UTC rendering, e.g. `2026-09-29 14:03:12 UTC`. */
export const formatUtcInstant = (value: InstantValue) => {
  const date = parseInstant(value);
  if (!date) return String(value);
  return `${date.toISOString().slice(0, utcSecondsLength).replace('T', ' ')} ${utcTimeZone}`;
};

const zoneOffsetMilliseconds = (epoch: number, timeZone: string) => {
  const parts = Object.fromEntries(
    formatter('en-US', timeZone, 'offset', {
      hourCycle: 'h23',
      year: 'numeric',
      month: 'numeric',
      day: 'numeric',
      hour: 'numeric',
      minute: 'numeric',
      second: 'numeric',
    })
      .formatToParts(epoch)
      .map((part) => [part.type, Number(part.value)]),
  );
  const wallClock = Date.UTC(
    parts.year,
    parts.month - 1,
    parts.day,
    parts.hour,
    parts.minute,
    parts.second,
  );
  return (
    wallClock - Math.floor(epoch / secondMilliseconds) * secondMilliseconds
  );
};

const localDateTimePattern =
  /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2}))?$/;

/**
 * Interprets a `datetime-local` value (`YYYY-MM-DDTHH:mm[:ss]`) as wall-clock
 * time in `timeZone` and returns the UTC ISO instant. A time skipped by a DST
 * transition moves forward by the gap; a repeated time resolves to the earlier
 * instant.
 */
export const zonedDateTimeToIso = (value: string, timeZone: string) => {
  const match = localDateTimePattern.exec(value);
  if (!match) return '';
  const [, year, month, day, hour, minute, second = '0'] = match;
  const wallClock = Date.UTC(
    Number(year),
    Number(month) - 1,
    Number(day),
    Number(hour),
    Number(minute),
    Number(second),
  );
  // Offsets a day either side bracket any transition affecting this time.
  const offsetBefore = zoneOffsetMilliseconds(
    wallClock - dayMilliseconds,
    timeZone,
  );
  const offsetAfter = zoneOffsetMilliseconds(
    wallClock + dayMilliseconds,
    timeZone,
  );
  const valid = [wallClock - offsetBefore, wallClock - offsetAfter].filter(
    (epoch) => epoch + zoneOffsetMilliseconds(epoch, timeZone) === wallClock,
  );
  const epoch = valid.length ? Math.min(...valid) : wallClock - offsetBefore;
  return new Date(epoch).toISOString();
};

/** The `datetime-local` input value showing `value` in `timeZone`. */
export const isoToZonedDateTime = (value: InstantValue, timeZone: string) => {
  const date = parseInstant(value);
  if (!date) return '';
  return new Date(
    date.getTime() + zoneOffsetMilliseconds(date.getTime(), timeZone),
  )
    .toISOString()
    .slice(0, localDateTimeMinutesLength);
};
