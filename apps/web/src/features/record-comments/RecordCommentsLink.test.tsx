// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { RecordCommentsLink } from './RecordCommentsLink';

vi.mock('@tanstack/react-router', () => ({
  createLink:
    () =>
    ({
      'aria-label': ariaLabel,
      children,
      hash,
      params,
      to,
    }: {
      'aria-label'?: string;
      children: ReactNode;
      hash: string;
      params: { recordId: string };
      to: string;
    }) => (
      <a
        aria-label={ariaLabel}
        href={`${to.replace('$recordId', params.recordId)}#${hash}`}
      >
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

describe('RecordCommentsLink', () => {
  it('links the comment count to the comments on the record page', async () => {
    const recordId = '123e4567-e89b-12d3-a456-426614174001';
    render(
      <QueryClientProvider client={new QueryClient()}>
        <RecordCommentsLink recordId={recordId} />
      </QueryClientProvider>,
    );

    const link = await screen.findByRole('link', { name: '3 comments' });
    expect(link.textContent).toBe('3');
    expect(link.getAttribute('href')).toBe(`/records/${recordId}#comments`);
  });
});
