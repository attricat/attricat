import { describe, expect, it } from 'vitest';
import type { Finding } from '../rules/schemas';
import { fieldFindings, worstSeverity } from './fieldDiagnostics';

const finding = (overrides: Partial<Finding>): Finding => ({
  id: crypto.randomUUID(),
  rule_id: crypto.randomUUID(),
  rule_version: 1,
  record_id: crypto.randomUUID(),
  context_id: null,
  severity: 'warning',
  message: 'Something is wrong',
  evidence: {},
  state: 'open',
  acknowledged_at: null,
  resolved_at: null,
  created_at: '2026-10-20T13:54:17Z',
  updated_at: '2026-10-20T13:54:17Z',
  ...overrides,
});

const polishContextId = crypto.randomUUID();
const shown = { contextCode: 'cars_pl', contextId: polishContextId };

describe('fieldFindings', () => {
  it('keeps a context finding in its own context', () => {
    const polish = finding({
      context_id: polishContextId,
      evidence: { attribute_code: 'meta_description', contexts: ['cars_pl'] },
    });

    expect(fieldFindings([polish], 'meta_description', shown)).toEqual([
      polish,
    ]);
    expect(
      fieldFindings([polish], 'meta_description', {
        contextCode: 'default',
        contextId: crypto.randomUUID(),
      }),
    ).toEqual([]);
  });

  it('shows a record-wide finding in the contexts it fails in', () => {
    const recordWide = finding({
      evidence: { attribute_code: 'listing_status', contexts: ['cars_pl'] },
    });

    expect(fieldFindings([recordWide], 'listing_status', shown)).toEqual([
      recordWide,
    ]);
    expect(
      fieldFindings([recordWide], 'listing_status', {
        contextCode: 'cars_de',
      }),
    ).toEqual([]);
  });

  it('skips findings about other fields or without an attribute', () => {
    const other = finding({ evidence: { attribute_code: 'title' } });
    const unnamed = finding({ evidence: { contexts: ['cars_pl'] } });

    expect(fieldFindings([other, unnamed], 'description', shown)).toEqual([]);
  });
});

describe('worstSeverity', () => {
  it('picks the most severe finding', () => {
    expect(
      worstSeverity([
        finding({ severity: 'info' }),
        finding({ severity: 'error' }),
        finding({ severity: 'warning' }),
      ]),
    ).toBe('error');
    expect(worstSeverity([])).toBeUndefined();
  });
});
