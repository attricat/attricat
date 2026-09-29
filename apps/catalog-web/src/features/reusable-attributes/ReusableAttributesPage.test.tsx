// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import {
  createReusableAttributeGroup,
  listReusableAttributeGroups,
  listReusableAttributes,
} from './api';
import '../../i18n';
import { ReusableAttributesPage } from './ReusableAttributesPage';

vi.mock('@tanstack/react-router', async () => {
  const { forwardRef, createElement } = await import('react');
  const Link = forwardRef<
    HTMLAnchorElement,
    React.ComponentProps<'a'> & { to?: string }
  >(({ to, children, ...props }, ref) => (
    <a {...props} href={to} ref={ref}>
      {children}
    </a>
  ));
  return {
    Link,
    createLink: (Component: React.ElementType) =>
      forwardRef((props, ref) =>
        createElement(Component, { ...props, ref, component: Link }),
      ),
    useNavigate: () => vi.fn(),
  };
});
vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  listReusableAttributes: vi.fn(),
  listReusableAttributeGroups: vi.fn(),
  createReusableAttributeGroup: vi.fn(),
}));

const renderPage = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ReusableAttributesPage />
    </QueryClientProvider>,
  );

describe('ReusableAttributesPage', () => {
  it('renders Edit as a single link rather than nesting a link in a button', async () => {
    vi.mocked(listReusableAttributes).mockResolvedValue([
      {
        id: 'revision',
        definition_id: 'definition',
        name: 'Title',
        namespace: 'catalog',
        code: 'title',
        version: 1,
        status: 'published',
        value_type: 'string',
      },
    ] as never);
    vi.mocked(listReusableAttributeGroups).mockResolvedValue([]);
    renderPage();
    const edit = await screen.findByRole('link', { name: 'Edit' });
    expect(edit.querySelector('a, button')).toBeNull();
  });

  it('prevents closing a group dialog while its create request is pending', async () => {
    vi.mocked(listReusableAttributes).mockResolvedValue([
      {
        id: 'revision',
        definition_id: 'definition',
        name: 'Title',
        namespace: 'catalog',
        code: 'title',
        version: 1,
        status: 'published',
        value_type: 'string',
      },
    ] as never);
    vi.mocked(listReusableAttributeGroups).mockResolvedValue([]);
    vi.mocked(createReusableAttributeGroup).mockImplementation(
      () => new Promise(() => {}),
    );
    renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'New group' }));
    fireEvent.change(screen.getByRole('textbox', { name: 'Code' }), {
      target: { value: 'group' },
    });
    fireEvent.change(screen.getByRole('textbox', { name: 'Name' }), {
      target: { value: 'Group' },
    });
    fireEvent.mouseDown(
      screen.getByRole('combobox', { name: 'Published revisions' }),
    );
    fireEvent.click(
      await screen.findByRole('option', { name: /catalog:title/ }),
    );
    fireEvent.keyDown(screen.getByRole('listbox'), { key: 'Escape' });
    fireEvent.click(screen.getByRole('button', { name: 'Create group' }));
    await waitFor(() =>
      expect(createReusableAttributeGroup).toHaveBeenCalledOnce(),
    );
    const cancel = screen.getByRole('button', { name: 'Cancel' });
    expect(cancel.hasAttribute('disabled')).toBe(true);
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(screen.getByRole('dialog')).toBeTruthy();
  });
});
