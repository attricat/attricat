import { afterEach, describe, expect, it, vi } from 'vitest';
import { requestOffsetPage } from '../../api/request';
import { listFindingsPage, listRuleRuns } from './api';
import { FINDINGS_PAGE_SIZE } from './constants';

vi.mock('../../api/request', () => ({
  jsonRequest: vi.fn(),
  request: vi.fn(),
  requestOffsetPage: vi.fn(() => Promise.resolve({ items: [] })),
}));
import { request } from '../../api/request';

const id = '123e4567-e89b-42d3-a456-426614174000';
const requestedQuery = () =>
  new URL(
    vi.mocked(requestOffsetPage).mock.calls.at(-1)![0],
    'http://localhost',
  ).searchParams;

describe('listFindingsPage', () => {
  afterEach(() => vi.clearAllMocks());

  it('lists unresolved findings unless a state is chosen', async () => {
    await listFindingsPage({});
    expect(Object.fromEntries(requestedQuery())).toEqual({
      limit: String(FINDINGS_PAGE_SIZE),
      offset: '0',
      unresolved: 'true',
    });
    await listFindingsPage({ state: 'all' }, 50);
    expect(Object.fromEntries(requestedQuery())).toEqual({
      limit: String(FINDINGS_PAGE_SIZE),
      offset: '50',
    });
  });

  it('sends every filter under its API name', async () => {
    await listFindingsPage({
      state: 'acknowledged',
      severity: 'critical',
      rule: id,
      blueprint: id,
      context: id,
    });
    expect(Object.fromEntries(requestedQuery())).toEqual({
      limit: String(FINDINGS_PAGE_SIZE),
      offset: '0',
      state: 'acknowledged',
      severity: 'critical',
      rule_id: id,
      blueprint_id: id,
      context_id: id,
    });
  });
});

describe('listRuleRuns', () => {
  afterEach(() => vi.clearAllMocks());

  it('lists runs over every record unless another scope is chosen', async () => {
    const query = () =>
      new URL(vi.mocked(request).mock.calls.at(-1)![0], 'http://localhost')
        .searchParams;
    await listRuleRuns({});
    expect(Object.fromEntries(query())).toEqual({ scope: 'all_records' });
    await listRuleRuns({ rule: id, scope: 'record' });
    expect(Object.fromEntries(query())).toEqual({
      rule_id: id,
      scope: 'record',
    });
    await listRuleRuns({ scope: 'any' });
    expect(Object.fromEntries(query())).toEqual({});
  });
});
