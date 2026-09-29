// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { createReusableAttribute, listReusableAttributes } from './api';
import '../../i18n';
import { ReusableAttributeEditorPage } from './ReusableAttributeEditorPage';

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
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
  createReusableAttribute: vi.fn(),
  listReusableAttributes: vi.fn(),
}));

describe('ReusableAttributeEditorPage', () => {
  it('freezes the definition and navigation during save', async () => {
    vi.mocked(listReusableAttributes).mockResolvedValue([]);
    vi.mocked(createReusableAttribute).mockImplementation(
      () => new Promise(() => {}),
    );
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ReusableAttributeEditorPage />
      </QueryClientProvider>,
    );
    fireEvent.change(screen.getByRole('textbox', { name: 'Definition' }), {
      target: { value: 'code = "saved"' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create attribute' }));
    await waitFor(() =>
      expect(createReusableAttribute).toHaveBeenCalledWith({
        definition: 'code = "saved"',
      }),
    );
    expect(
      screen
        .getByRole('textbox', { name: 'Definition' })
        .hasAttribute('disabled'),
    ).toBe(true);
    expect(
      screen.getByRole('button', { name: 'Cancel' }).hasAttribute('disabled'),
    ).toBe(true);
  });
});
