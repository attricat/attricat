// @vitest-environment jsdom
import { render } from '@testing-library/react';
import type { ComponentType, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { Route as ChangelogRoute } from '../../routes/manage/rules/$ruleId/changelog';
import { RULE_PAGE_CHANGELOG } from './constants';
import { RulePage } from './RulePage';

vi.mock('./RulePage', () => ({ RulePage: vi.fn(() => null) }));
vi.mock('../auth/RequireCapability', () => ({
  RequireCapability: ({ children }: { children: ReactNode }) => children,
}));

const ruleId = '00000000-0000-4000-8000-000000000003';

describe('rule changelog route', () => {
  it('compares the revisions its URL names and ignores malformed ones', async () => {
    const search = ChangelogRoute.options.validateSearch as (
      input: Record<string, unknown>,
    ) => unknown;
    expect(search({ from: '2', to: 'latest' })).toEqual({
      from: 2,
      to: undefined,
    });
    vi.spyOn(ChangelogRoute, 'useParams').mockReturnValue({ ruleId } as never);
    vi.spyOn(ChangelogRoute, 'useSearch').mockReturnValue({
      from: 1,
      to: 3,
    } as never);
    // The router plugin splits the component into a lazily loaded chunk.
    const Component = ChangelogRoute.options.component as ComponentType & {
      preload?: () => Promise<void>;
    };
    await Component.preload?.();
    render(<Component />);
    expect(vi.mocked(RulePage).mock.calls[0]?.[0]).toEqual({
      comparison: { from: 1, to: 3 },
      ruleId,
      section: RULE_PAGE_CHANGELOG,
    });
  });
});
