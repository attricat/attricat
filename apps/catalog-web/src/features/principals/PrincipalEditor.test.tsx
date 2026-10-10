// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { Attribute } from '../records/api';
import { ScalarAttributeEditor } from '../records/components/ScalarAttributeEditor';
import { AttributeValue } from '../views/components/values/AttributeValue';
import { getDirectory } from './api';

vi.mock('./api', () => ({ getDirectory: vi.fn() }));

const ada = '6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60';
const former = '7b2f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f61';
const team = '8c3f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f62';
const assignee: Attribute = {
  code: 'assignee',
  name: 'Assignee',
  value_type: 'string',
  value_schema: {
    type: 'string',
    'x-attricat-principal': { version: 1, kinds: ['user', 'team'] },
  },
};

const withClient = (node: ReactNode) => {
  vi.mocked(getDirectory).mockResolvedValue({
    users: [
      { id: ada, display_name: 'Ada', email: 'ada@example.test', active: true },
      {
        id: former,
        display_name: 'Former',
        email: 'former@example.test',
        active: false,
      },
    ],
    teams: [{ id: team, code: 'qa', name: 'Quality', deleted: false }],
  });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>{node}</QueryClientProvider>,
  );
};

afterEach(cleanup);

describe('user-or-team attributes', () => {
  it('pick active users and teams and store their references', async () => {
    const onChange = vi.fn();
    withClient(
      <ScalarAttributeEditor
        attribute={assignee}
        value=""
        disabled={false}
        onChange={onChange}
      />,
    );
    const input = screen.getByRole('combobox', { name: 'Assignee' });
    fireEvent.mouseDown(input);
    const listbox = await screen.findByRole('listbox');
    await within(listbox).findByText('Quality');
    expect(within(listbox).queryByText('Former')).toBeNull();
    fireEvent.click(within(listbox).getByText('Quality'));
    expect(onChange).toHaveBeenCalledWith(`team:${team}`);
  });

  it('keep a saved assignee who has left and explain it', async () => {
    withClient(
      <AttributeValue attribute={assignee} value={`user:${former}`} />,
    );
    expect(await screen.findByText('Former')).toBeTruthy();
    expect(screen.getByText('No longer an active member')).toBeTruthy();
  });

  it('report a failed directory load instead of an unknown assignee', async () => {
    vi.mocked(getDirectory).mockRejectedValue(new Error('Directory down'));
    render(
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <ScalarAttributeEditor
          attribute={assignee}
          value={`user:${ada}`}
          disabled={false}
          onChange={vi.fn()}
        />
      </QueryClientProvider>,
    );
    expect(
      await screen.findByText('Users and teams could not be loaded'),
    ).toBeTruthy();
    expect(screen.queryByText(/Unknown user or team/)).toBeNull();
  });
});
