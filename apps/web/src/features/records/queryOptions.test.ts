import { QueryClient } from '@tanstack/react-query';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ruleQueryKeys } from '../rules/queryKeys';
import { recordDiagnosticFindingsOptions } from './queryOptions';

const recordId = '123e4567-e89b-12d3-a456-426614174000';

const finding = (id: number) => ({
  id: `00000000-0000-4000-8000-${String(id).padStart(12, '0')}`,
  rule_id: '223e4567-e89b-12d3-a456-426614174000',
  rule_version: 1,
  record_id: recordId,
  context_id: null,
  severity: 'warning',
  message: "'title' is required",
  evidence: { attribute_code: 'title' },
  state: 'open',
  acknowledged_at: null,
  resolved_at: null,
  created_at: '2026-10-20T13:54:17Z',
  updated_at: '2026-10-20T13:54:17Z',
});

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => {
  fetchMock.mockReset();
});

const page = (items: unknown[], nextOffset?: number) => ({
  ok: true,
  headers: new Headers(
    nextOffset === undefined ? {} : { 'x-next-offset': String(nextOffset) },
  ),
  json: () => Promise.resolve(items),
});

describe('recordDiagnosticFindingsOptions', () => {
  it('loads every page of the record’s unresolved findings', async () => {
    fetchMock
      .mockResolvedValueOnce(page([finding(1), finding(2)], 2))
      .mockResolvedValueOnce(page([finding(3)]));

    const findings = await new QueryClient().fetchQuery(
      recordDiagnosticFindingsOptions(recordId),
    );

    expect(findings.map((item) => item.id)).toEqual([
      finding(1).id,
      finding(2).id,
      finding(3).id,
    ]);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(String(fetchMock.mock.calls[1]?.[0])).toContain('offset=2');
  });

  it('stops when the next offset does not move on', async () => {
    fetchMock.mockResolvedValue(page([finding(1)], 0));

    await expect(
      new QueryClient().fetchQuery(recordDiagnosticFindingsOptions(recordId)),
    ).resolves.toHaveLength(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('is refreshed with the record’s findings', () => {
    expect(
      recordDiagnosticFindingsOptions(recordId).queryKey.slice(
        0,
        ruleQueryKeys.findings(recordId).length,
      ),
    ).toEqual([...ruleQueryKeys.findings(recordId)]);
  });
});
