// @vitest-environment jsdom
import { render } from '@testing-library/react';
import type { ComponentType, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { Route as RevisionEditorRoute } from '../../routes/manage/rules/$ruleId/revisions/$version/new';
import { RuleEditorPage } from './RuleEditorPage';

vi.mock('./RuleEditorPage', () => ({ RuleEditorPage: vi.fn(() => null) }));
vi.mock('../auth/RequireCapability', () => ({
  RequireCapability: ({ children }: { children: ReactNode }) => children,
}));

const ruleId = '00000000-0000-4000-8000-000000000003';

describe('rule revision editor route', () => {
  it('opens the editor’s not-found state for a malformed version', async () => {
    vi.spyOn(RevisionEditorRoute, 'useParams').mockReturnValue({
      ruleId,
      version: 'latest',
    } as never);
    // The router plugin splits the component into a lazily loaded chunk.
    const Component = RevisionEditorRoute.options.component as ComponentType & {
      preload?: () => Promise<void>;
    };
    await Component.preload?.();
    render(<Component />);
    expect(vi.mocked(RuleEditorPage).mock.calls[0]?.[0]).toEqual({
      ruleId,
      sourceVersion: undefined,
    });
  });
});
