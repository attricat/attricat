// @vitest-environment jsdom
import {
  fireEvent,
  render as testingRender,
  screen,
  waitFor,
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { Attribute } from '../entities/api';
import {
  getFileMetadata,
  updateFileReferences,
  uploadFiles,
  uploadStagedFiles,
} from './api';
import { FileAttributeEditor } from './FileAttributeEditor';
import {
  QueuedFileUploadsContext,
  useQueuedFileUploads,
} from './queuedFileUploads';
import type { FileMetadata } from './schemas';

vi.mock('./api', () => ({
  uploadFiles: vi.fn(),
  uploadStagedFiles: vi.fn(),
  updateFileReferences: vi.fn(),
  getFileMetadata: vi.fn().mockRejectedValue(new Error('Unavailable')),
  fileDownloadUrl: (id: string) => `/files/${id}`,
}));

const render = (ui: ReactElement) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return testingRender(ui, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  });
};

const attribute = {
  code: 'document',
  value_type: 'file',
  file_policy: {
    cardinality: 'many',
    ordered: false,
    allowed_mime_groups: [],
    allowed_extensions: [],
    purposes: [],
    image_only: false,
  },
} as Attribute;
const entityId = '123e4567-e89b-12d3-a456-426614174000';
const blueprintId = '123e4567-e89b-12d3-a456-426614174002';
const file = new File(['contents'], 'document.txt', { type: 'text/plain' });
const renderEditor = (files: FileMetadata[] = [], id?: string) =>
  render(
    <FileAttributeEditor
      attribute={attribute}
      contextId={null}
      disabled={false}
      entityId={id}
      files={files}
    />,
  );

beforeEach(() => {
  vi.mocked(uploadFiles).mockReset();
  vi.mocked(uploadStagedFiles).mockReset();
  vi.mocked(updateFileReferences).mockReset();
  vi.mocked(getFileMetadata).mockRejectedValue(new Error('Unavailable'));
});

describe('FileAttributeEditor', () => {
  it('queues at most one dropped file for a single-file attribute', () => {
    const single = {
      ...attribute,
      file_policy: { ...attribute.file_policy!, cardinality: 'one' as const },
    };
    render(
      <FileAttributeEditor
        attribute={single}
        contextId={null}
        disabled={false}
        entityId={entityId}
        files={[]}
      />,
    );
    fireEvent.drop(
      screen.getByRole('button', { name: 'Choose or drop files' })
        .parentElement!,
      {
        dataTransfer: {
          files: [
            file,
            new File(['more'], 'second.txt', { type: 'text/plain' }),
          ],
        },
      },
    );
    expect(screen.getByText('document.txt')).toBeTruthy();
    expect(screen.queryByText('second.txt')).toBeNull();
    expect(
      (
        screen.getByRole('button', {
          name: 'Choose or drop files',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    fireEvent.drop(
      screen.getByRole('button', { name: 'Choose or drop files' })
        .parentElement!,
      {
        dataTransfer: {
          files: [new File(['again'], 'third.txt', { type: 'text/plain' })],
        },
      },
    );
    expect(screen.queryByText('third.txt')).toBeNull();
  });

  it('does not show uploads from the previous entity after navigation', async () => {
    const metadata = {
      id: 'uploaded-id',
      filename: 'previous.txt',
      status: 'ready',
    } as FileMetadata;
    vi.mocked(uploadFiles).mockResolvedValue({ files: [metadata] } as Awaited<
      ReturnType<typeof uploadFiles>
    >);
    const view = renderEditor([], entityId);
    fireEvent.change(view.container.querySelector('input[type="file"]')!, {
      target: { files: [file] },
    });
    fireEvent.click(screen.getByRole('button', { name: /Upload 1/ }));
    expect(await screen.findByText('previous.txt')).toBeTruthy();
    view.rerender(
      <FileAttributeEditor
        attribute={attribute}
        contextId={null}
        disabled={false}
        entityId="another-entity"
        files={[]}
      />,
    );
    expect(screen.queryByText('previous.txt')).toBeNull();
  });

  it('does not queue dropped files until an entity exists', () => {
    renderEditor();
    fireEvent.drop(
      screen.getByRole('button', { name: 'Choose or drop files' })
        .parentElement!,
      {
        dataTransfer: { files: [file] },
      },
    );
    expect(screen.queryByText('document.txt')).toBeNull();
  });

  it('queues files to upload when a new record is created', async () => {
    vi.mocked(uploadStagedFiles).mockResolvedValue({
      files: [{ id: entityId }],
    } as unknown as Awaited<ReturnType<typeof uploadStagedFiles>>);
    const results: unknown[] = [];
    const Creating = () => {
      const queued = useQueuedFileUploads('product');
      return (
        <QueuedFileUploadsContext value={queued.queue}>
          <FileAttributeEditor
            attribute={attribute}
            contextId={null}
            disabled={false}
            files={[]}
          />
          <button
            onClick={() =>
              void queued
                .stageQueued(queued.queue.pending, blueprintId, 'context-id')
                .then((result) => results.push(result))
            }
            type="button"
          >
            Create
          </button>
        </QueuedFileUploadsContext>
      );
    };
    render(<Creating />);
    fireEvent.drop(
      screen.getByRole('button', { name: 'Choose or drop files' })
        .parentElement!,
      { dataTransfer: { files: [file] } },
    );
    expect(screen.getByText('document.txt')).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Upload 1/ })).toBeNull();
    expect(uploadStagedFiles).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() =>
      expect(results).toEqual([
        {
          failed: [],
          staged: [
            {
              attribute_code: 'document',
              context_id: 'context-id',
              file_ids: [entityId],
            },
          ],
        },
      ]),
    );
    expect(uploadStagedFiles).toHaveBeenCalledWith(
      expect.objectContaining({
        attributeCode: 'document',
        blueprintId,
        contextId: 'context-id',
        files: [file],
      }),
    );
    expect(uploadFiles).not.toHaveBeenCalled();
  });

  it('keeps a file that failed to upload before create, to retry or remove', async () => {
    vi.mocked(uploadStagedFiles).mockRejectedValue(new Error('Too large'));
    const Creating = () => {
      const queued = useQueuedFileUploads('product');
      return (
        <QueuedFileUploadsContext value={queued.queue}>
          <FileAttributeEditor
            attribute={attribute}
            contextId={null}
            disabled={false}
            files={[]}
          />
          <button
            onClick={() =>
              void queued.stageQueued(queued.queue.pending, blueprintId, null)
            }
            type="button"
          >
            Create
          </button>
        </QueuedFileUploadsContext>
      );
    };
    render(<Creating />);
    fireEvent.drop(
      screen.getByRole('button', { name: 'Choose or drop files' })
        .parentElement!,
      { dataTransfer: { files: [file] } },
    );
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Too large');
    // Retrying queues the file again for the next create.
    fireEvent.click(
      screen.getByRole('button', { name: /Retry.*document\.txt/ }),
    );
    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.getByText('document.txt')).toBeTruthy();
    fireEvent.click(
      screen.getByRole('button', { name: /Remove.*document\.txt/ }),
    );
    expect(screen.queryByText('document.txt')).toBeNull();
  });

  it('uploads each queued file only once when upload is clicked repeatedly', async () => {
    const view = renderEditor([], entityId);
    let finish!: (value: Awaited<ReturnType<typeof uploadFiles>>) => void;
    vi.mocked(uploadFiles).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    fireEvent.change(view.container.querySelector('input[type="file"]')!, {
      target: { files: [file] },
    });
    const upload = screen.getByRole('button', { name: /Upload 1/ });
    fireEvent.click(upload);
    fireEvent.click(upload);
    expect(uploadFiles).toHaveBeenCalledOnce();
    finish({ files: [] } as never);
    await waitFor(() => expect(screen.queryByText('document.txt')).toBeNull());
  });

  it('reflects refreshed server files instead of freezing the initial prop', () => {
    const view = renderEditor([], entityId);
    const metadata = {
      id: '123e4567-e89b-12d3-a456-426614174001',
      filename: 'server.txt',
      status: 'ready',
    } as FileMetadata;
    view.rerender(
      <FileAttributeEditor
        attribute={attribute}
        contextId={null}
        disabled={false}
        entityId={entityId}
        files={[metadata]}
      />,
    );
    expect(screen.getByText('server.txt')).toBeTruthy();
  });

  it('rejects unsafe images and removes queued previews without uploading', () => {
    URL.createObjectURL = vi.fn().mockReturnValue('blob:queued');
    URL.revokeObjectURL = vi.fn();
    const imageAttribute = {
      ...attribute,
      file_policy: { ...attribute.file_policy!, image_only: true },
    };
    const view = render(
      <FileAttributeEditor
        attribute={imageAttribute}
        contextId={null}
        disabled={false}
        entityId={entityId}
        files={[]}
      />,
    );
    fireEvent.change(view.container.querySelector('input[type="file"]')!, {
      target: {
        files: [
          new File(['png'], 'photo.png', { type: 'image/png' }),
          new File(['svg'], 'unsafe.svg', { type: 'image/svg+xml' }),
        ],
      },
    });
    expect(screen.getByRole('alert').textContent).toContain(
      'unsafe.svg was not added',
    );
    expect(URL.createObjectURL).toHaveBeenCalledOnce();
    fireEvent.click(
      screen.getByRole('button', {
        name: 'Remove photo.png from upload queue',
      }),
    );
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:queued');
    expect(screen.queryByRole('button', { name: /Upload 1/ })).toBeNull();
    expect(uploadFiles).not.toHaveBeenCalled();
  });

  it('blocks drop, retry and queued removal when disabled after an upload error', async () => {
    vi.mocked(uploadFiles).mockRejectedValue(new Error('Upload denied'));
    const view = renderEditor([], entityId);
    fireEvent.change(view.container.querySelector('input[type="file"]')!, {
      target: { files: [file] },
    });
    fireEvent.click(screen.getByRole('button', { name: /Upload 1/ }));
    expect(await screen.findByText('Upload denied')).toBeTruthy();
    view.rerender(
      <FileAttributeEditor
        attribute={attribute}
        contextId={null}
        disabled
        entityId={entityId}
        files={[]}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Retry document.txt' }));
    fireEvent.click(
      screen.getByRole('button', {
        name: 'Remove document.txt from upload queue',
      }),
    );
    fireEvent.drop(
      screen.getByRole('button', { name: 'Choose or drop files' })
        .parentElement!,
      {
        dataTransfer: { files: [new File(['x'], 'blocked.txt')] },
      },
    );
    expect(screen.queryByText('blocked.txt')).toBeNull();
    expect(screen.getByText('document.txt')).toBeTruthy();
    expect(uploadFiles).toHaveBeenCalledOnce();
  });

  it('sends compare-and-swap references and keeps attachments on conflicts', async () => {
    const images = [
      { id: entityId, filename: 'a.png', status: 'ready' },
      {
        id: '123e4567-e89b-12d3-a456-426614174001',
        filename: 'b.png',
        status: 'ready',
      },
    ] as FileMetadata[];
    vi.mocked(updateFileReferences).mockRejectedValue(
      new Error('References changed; refresh'),
    );
    render(
      <FileAttributeEditor
        attribute={{
          ...attribute,
          file_policy: {
            ...attribute.file_policy!,
            ordered: true,
            image_only: true,
          },
        }}
        contextId={null}
        disabled={false}
        entityId={entityId}
        files={images}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Move a.png later' }));
    expect(await screen.findByText('References changed; refresh')).toBeTruthy();
    expect(updateFileReferences).toHaveBeenCalledWith(entityId, 'document', {
      context_id: null,
      expected_file_ids: images.map((file) => file.id),
      file_ids: [images[1].id, images[0].id],
    });
    expect(
      screen
        .getAllByRole('button', { name: /^Preview/ })
        .map((button) => button.getAttribute('aria-label')),
    ).toEqual(['Preview a.png', 'Preview b.png']);
  });
});
