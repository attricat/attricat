// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import {
  createReusableAttributeGroup,
  deleteReusableAttributeGroup,
  listReusableAttributeGroups,
  listReusableAttributes,
  updateReusableAttributeGroup,
} from './api';
import '../../i18n';
import { ReusableAttributesPage } from './ReusableAttributesPage';

vi.mock('../auth/useCan', () => ({
  useAccess: () => ({ can: () => true, isPending: false }),
  useCan: () => true,
  useCanOn: () => true,
}));

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
  deleteReusableAttributeGroup: vi.fn(),
  updateReusableAttributeGroup: vi.fn(),
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
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Attributes' }));
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

  const titleRevision = (version: number, status: string) => ({
    id: `revision-${version}`,
    definition_id: 'definition',
    name: 'Title',
    namespace: 'catalog',
    code: 'title',
    version,
    status,
    value_type: 'string',
  });
  const specs = {
    id: 'group',
    code: 'specs',
    name: 'Specs',
    position: 2,
    reusable_attribute_codes: ['catalog:title'],
  };

  it('lists an attribute once for a group, however many revisions it has', async () => {
    vi.mocked(listReusableAttributes).mockResolvedValue([
      titleRevision(1, 'published'),
      titleRevision(2, 'published'),
      titleRevision(3, 'draft'),
      {
        ...titleRevision(1, 'draft'),
        id: 'unpublished',
        definition_id: 'unpublished',
        code: 'unpublished',
      },
    ] as never);
    vi.mocked(listReusableAttributeGroups).mockResolvedValue([]);
    renderPage();
    await screen.findByText('catalog:unpublished');
    fireEvent.click(screen.getByRole('button', { name: 'New group' }));
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Attributes' }));
    const options = await screen.findAllByRole('option');
    expect(options.map((option) => option.textContent)).toEqual([
      'catalog:title',
    ]);
  });

  it('edits a group without changing its code', async () => {
    vi.mocked(listReusableAttributes).mockResolvedValue([
      titleRevision(1, 'published'),
    ] as never);
    vi.mocked(listReusableAttributeGroups).mockResolvedValue([specs]);
    vi.mocked(updateReusableAttributeGroup).mockResolvedValue(specs);
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Edit group Specs' }),
    );
    const code = screen.getByRole('textbox', { name: 'Code' });
    expect(code.hasAttribute('disabled')).toBe(true);
    fireEvent.change(screen.getByRole('textbox', { name: 'Name' }), {
      target: { value: 'Renamed' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));
    await waitFor(() =>
      expect(updateReusableAttributeGroup).toHaveBeenCalledWith('group', {
        name: 'Renamed',
        position: 2,
        reusable_attribute_codes: ['catalog:title'],
      }),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('asks before deleting a group', async () => {
    vi.mocked(listReusableAttributes).mockResolvedValue([] as never);
    vi.mocked(listReusableAttributeGroups).mockResolvedValue([specs]);
    vi.mocked(deleteReusableAttributeGroup).mockResolvedValue(undefined);
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Delete group Specs' }),
    );
    expect(deleteReusableAttributeGroup).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Delete group' }));
    await waitFor(() =>
      expect(deleteReusableAttributeGroup).toHaveBeenCalledWith('group'),
    );
  });
});
