// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { EntityCommentsLink } from './EntityCommentsLink';

vi.mock('@tanstack/react-router', () => ({
  createLink:
    () =>
    ({
      children,
      hash,
      params,
      to,
    }: {
      children: ReactNode;
      hash: string;
      params: { entityId: string };
      to: string;
    }) => (
      <a href={`${to.replace('$entityId', params.entityId)}#${hash}`}>
        {children}
      </a>
    ),
}));
vi.mock('../auth/api', () => ({
  currentSession: () =>
    Promise.resolve({
      user_id: '123e4567-e89b-12d3-a456-426614174002',
      workspace_id: '123e4567-e89b-12d3-a456-426614174003',
    }),
}));
vi.mock('./api', () => ({
  getCommentCount: () => Promise.resolve({ count: 3 }),
}));

describe('EntityCommentsLink', () => {
  it('links the comment count to the comments on the entity page', async () => {
    const entityId = '123e4567-e89b-12d3-a456-426614174001';
    render(
      <QueryClientProvider client={new QueryClient()}>
        <EntityCommentsLink entityId={entityId} />
      </QueryClientProvider>,
    );

    const link = await screen.findByRole('link', { name: '3 comments' });
    expect(link.getAttribute('href')).toBe(`/entities/${entityId}#comments`);
  });
});
