// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { createBlueprint } from './api';
import { BlueprintEditorPage } from './BlueprintEditorPage';

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
  createLink: <T,>(component: T) => component,
}));
vi.mock('@monaco-editor/react', () => ({
  Editor: ({
    onChange,
    options,
  }: {
    onChange: (value: string) => void;
    options: { readOnly: boolean };
  }) => (
    <textarea
      aria-label="Definition"
      disabled={options.readOnly}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
}));
vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  createBlueprint: vi.fn(),
}));

describe('BlueprintEditorPage', () => {
  it('locks its editor and template controls while saving a draft', async () => {
    vi.mocked(createBlueprint).mockImplementation(() => new Promise(() => {}));
    render(
      <QueryClientProvider client={new QueryClient()}>
        <BlueprintEditorPage />
      </QueryClientProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    fireEvent.change(screen.getByRole('textbox', { name: 'Definition' }), {
      target: { value: 'code = "saved"' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));
    await waitFor(() =>
      expect(createBlueprint).toHaveBeenCalledWith('code = "saved"'),
    );
    expect(
      screen
        .getByRole('textbox', { name: 'Definition' })
        .hasAttribute('disabled'),
    ).toBe(true);
    expect(
      screen.getByRole('button', { name: 'Examples' }).hasAttribute('disabled'),
    ).toBe(true);
  });
});
