import { describe, expect, it } from 'vitest';
import {
  browserTimeZone,
  formatCalendarDate,
  formatInstant,
  formatUtcInstant,
  isoToZonedDateTime,
  isValidTimeZone,
  resolveTimeZone,
  supportedTimeZones,
  timeZoneLabel,
  zonedDateTimeToIso,
} from './instantFormat';

const instant = '2026-09-29T14:03:12.345Z';

describe('formatInstant', () => {
  it('renders the same instant in the requested zone', () => {
    expect(
      formatInstant(instant, { locale: 'en-GB', timeZone: 'Europe/Warsaw' }),
    ).toBe('29 Sept 2026, 16:03');
    expect(
      formatInstant(instant, { locale: 'en-GB', timeZone: 'Asia/Tokyo' }),
    ).toBe('29 Sept 2026, 23:03');
    expect(
      formatInstant(instant, {
        locale: 'en-GB',
        timeZone: 'America/Los_Angeles',
        style: 'date',
      }),
    ).toBe('29 Sept 2026');
  });

  it('crosses the calendar day with the zone', () => {
    expect(
      formatInstant('2026-01-01T02:00:00Z', {
        locale: 'en-GB',
        timeZone: 'America/New_York',
        style: 'date',
      }),
    ).toBe('31 Dec 2025');
  });

  it('follows DST around the Europe/Warsaw spring transition', () => {
    const options = {
      locale: 'en-GB',
      timeZone: 'Europe/Warsaw',
      style: 'time',
    } as const;
    // 2026-03-29 01:00 UTC is when CET (+1) becomes CEST (+2).
    expect(formatInstant('2026-03-29T00:59:59Z', options)).toBe('01:59:59');
    expect(formatInstant('2026-03-29T01:00:00Z', options)).toBe('03:00:00');
  });

  it('appends the zone name only when asked', () => {
    expect(
      formatInstant(instant, {
        locale: 'en-GB',
        timeZone: 'Europe/Warsaw',
        showTimeZone: true,
      }),
    ).toBe('29 Sept 2026, 16:03 CEST');
    expect(
      timeZoneLabel('2026-01-15T12:00:00Z', 'en-GB', 'Europe/Warsaw'),
    ).toBe('CET');
  });

  it('uses the active language', () => {
    expect(
      formatInstant(instant, {
        locale: 'pl',
        timeZone: 'UTC',
        style: 'date',
      }),
    ).toBe('29 wrz 2026');
  });

  it('keeps malformed values visible instead of throwing', () => {
    expect(formatInstant('not a date', { locale: 'en', timeZone: 'UTC' })).toBe(
      'not a date',
    );
    expect(formatUtcInstant('not a date')).toBe('not a date');
  });
});

describe('formatUtcInstant', () => {
  it('renders the exact UTC time independently of locale and zone', () => {
    expect(formatUtcInstant(instant)).toBe('2026-09-29 14:03:12 UTC');
    expect(formatUtcInstant('2026-09-29T16:03:12+02:00')).toBe(
      '2026-09-29 14:03:12 UTC',
    );
  });
});

describe('formatCalendarDate', () => {
  it('never shifts a calendar date into another zone', () => {
    expect(formatCalendarDate('2026-03-01', 'en-GB')).toBe('1 Mar 2026');
  });
});

describe('time zone resolution', () => {
  it('prefers a valid stored zone and falls back to the browser zone', () => {
    expect(resolveTimeZone('Europe/Warsaw')).toBe('Europe/Warsaw');
    expect(resolveTimeZone('UTC')).toBe('UTC');
    expect(resolveTimeZone(null)).toBe(browserTimeZone());
    expect(resolveTimeZone(undefined)).toBe(browserTimeZone());
    expect(resolveTimeZone('Mars/Olympus')).toBe(browserTimeZone());
  });

  it('validates IANA names', () => {
    expect(isValidTimeZone('Europe/Warsaw')).toBe(true);
    expect(isValidTimeZone('')).toBe(false);
    expect(isValidTimeZone('Mars/Olympus')).toBe(false);
  });

  it('always offers UTC', () => {
    expect(supportedTimeZones()).toContain('UTC');
    expect(supportedTimeZones()).toContain('Europe/Warsaw');
  });
});

describe('zonedDateTimeToIso', () => {
  it('interprets datetime-local input in the given zone', () => {
    expect(zonedDateTimeToIso('2026-09-29T16:03', 'Europe/Warsaw')).toBe(
      '2026-09-29T14:03:00.000Z',
    );
    expect(zonedDateTimeToIso('2026-09-29T16:03', 'UTC')).toBe(
      '2026-09-29T16:03:00.000Z',
    );
    expect(zonedDateTimeToIso('2026-01-15T09:30:15', 'America/New_York')).toBe(
      '2026-01-15T14:30:15.000Z',
    );
    expect(zonedDateTimeToIso('', 'UTC')).toBe('');
  });

  it('moves times skipped by DST forward by the gap', () => {
    // 02:30 does not exist in Warsaw on 2026-03-29; it becomes 03:30 CEST.
    expect(zonedDateTimeToIso('2026-03-29T02:30', 'Europe/Warsaw')).toBe(
      '2026-03-29T01:30:00.000Z',
    );
  });

  it('resolves times repeated by DST to the earlier instant', () => {
    // 02:30 happens twice in Warsaw on 2026-10-25: CEST first, then CET.
    expect(zonedDateTimeToIso('2026-10-25T02:30', 'Europe/Warsaw')).toBe(
      '2026-10-25T00:30:00.000Z',
    );
  });

  it('round-trips with isoToZonedDateTime', () => {
    expect(isoToZonedDateTime(instant, 'Europe/Warsaw')).toBe(
      '2026-09-29T16:03',
    );
    expect(
      zonedDateTimeToIso(
        isoToZonedDateTime('2026-10-25T03:30:00Z', 'Europe/Warsaw'),
        'Europe/Warsaw',
      ),
    ).toBe('2026-10-25T03:30:00.000Z');
  });
});
