// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import '../../i18n';
import { listContexts } from '../contexts/api';
import { uploadStagedFiles } from '../files/api';
import { useQueuedFileUploadsContext } from '../files/queuedFileUploads';
import { createRecord, getBlueprintByCode } from './api';
import { CreateRecordPage } from './CreateRecordPage';

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
}));
vi.mock('../contexts/api', () => ({ listContexts: vi.fn() }));
vi.mock('../files/api', () => ({ uploadStagedFiles: vi.fn() }));
vi.mock('./api', () => ({
  createRecord: vi.fn(),
  getBlueprintByCode: vi.fn(),
}));
vi.mock('./components/RecordPage', () => ({
  RecordPage: ({ children }: { children: ReactNode }) => <>{children}</>,
}));
// Stands in for the form: queues a file as a file editor would and submits.
vi.mock('./components/RecordForm', () => ({
  RecordForm: ({
    error,
    onSubmit,
  }: {
    error?: Error | null;
    onSubmit: (values: { values: []; relationships: [] }) => void;
  }) => {
    const queue = useQueuedFileUploadsContext();
    return (
      <>
        <button
          onClick={() =>
            queue?.update('photo', (items) => [
              ...items,
              {
                file: new File(['png'], 'photo.png', { type: 'image/png' }),
                id: 'queued-photo',
                progress: 0,
              },
            ])
          }
          type="button"
        >
          Queue photo
        </button>
        <button
          onClick={() => onSubmit({ values: [], relationships: [] })}
          type="button"
        >
          Create record
        </button>
        {error && <p role="alert">{error.message}</p>}
      </>
    );
  },
}));

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const fileId = '123e4567-e89b-12d3-a456-426614174002';
const recordId = '123e4567-e89b-12d3-a456-426614174003';

beforeEach(() => {
  vi.mocked(createRecord).mockReset();
  vi.mocked(uploadStagedFiles).mockReset();
  navigate.mockReset();
  vi.mocked(listContexts).mockResolvedValue([
    { id: contextId, code: 'default' },
  ] as unknown as Awaited<ReturnType<typeof listContexts>>);
  vi.mocked(getBlueprintByCode).mockResolvedValue({
    blueprint: { id: blueprintId, code: 'product', version: 2 },
    attributes: [],
  } as unknown as Awaited<ReturnType<typeof getBlueprintByCode>>);
});

const renderPage = async () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <CreateRecordPage search={{ blueprint: 'product' }} />
    </QueryClientProvider>,
  );
  // The queue is scoped to the loaded blueprint.
  await vi.waitFor(() => expect(getBlueprintByCode).toHaveBeenCalled());
  await vi.waitFor(() => expect(listContexts).toHaveBeenCalled());
  // Let both queries settle so the queue's scope stops changing.
  await new Promise((resolve) => setTimeout(resolve));
  return userEvent.setup();
};

it('uploads queued files first and creates the record with them', async () => {
  vi.mocked(uploadStagedFiles).mockResolvedValue({
    files: [{ id: fileId }],
  } as unknown as Awaited<ReturnType<typeof uploadStagedFiles>>);
  vi.mocked(createRecord).mockResolvedValue({
    id: recordId,
  } as unknown as Awaited<ReturnType<typeof createRecord>>);
  const user = await renderPage();
  await user.click(screen.getByRole('button', { name: 'Queue photo' }));
  await user.click(screen.getByRole('button', { name: 'Create record' }));

  await vi.waitFor(() => expect(createRecord).toHaveBeenCalled());
  expect(uploadStagedFiles).toHaveBeenCalledWith(
    expect.objectContaining({
      blueprintId,
      attributeCode: 'photo',
      contextId,
    }),
  );
  expect(vi.mocked(createRecord).mock.calls[0][0].files).toEqual([
    { attribute_code: 'photo', context_id: contextId, file_ids: [fileId] },
  ]);
  await vi.waitFor(() =>
    expect(navigate).toHaveBeenCalledWith(
      expect.objectContaining({ params: { recordId } }),
    ),
  );
});

it('does not create the record when a file fails to upload', async () => {
  vi.mocked(uploadStagedFiles).mockRejectedValue(new Error('Too large'));
  const user = await renderPage();
  await user.click(screen.getByRole('button', { name: 'Queue photo' }));
  await user.click(screen.getByRole('button', { name: 'Create record' }));

  expect((await screen.findByRole('alert')).textContent).toBe(
    'photo.png (Too large) failed to upload, so the record was not created. Retry or remove the file, then create the record again.',
  );
  expect(createRecord).not.toHaveBeenCalled();
  expect(navigate).not.toHaveBeenCalled();
});
