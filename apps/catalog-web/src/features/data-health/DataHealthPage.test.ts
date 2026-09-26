import { describe, expect, it } from 'vitest';
import { formatDataHealthDate } from './dateFormat';

describe('formatDataHealthDate', () => {
  it('uses the active language and localized fallback', () => {
    const date = '2025-01-15T12:00:00Z';

    expect(formatDataHealthDate(date, 'pl', 'Nigdy')).toBe(
      new Intl.DateTimeFormat('pl', { dateStyle: 'medium' }).format(
        new Date(date),
      ),
    );
    expect(formatDataHealthDate(null, 'pl', 'Nigdy')).toBe('Nigdy');
  });
});
