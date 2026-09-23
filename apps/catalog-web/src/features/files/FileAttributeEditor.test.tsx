// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { Attribute } from '../entities/api';
import { uploadFiles } from './api';
import { FileAttributeEditor } from './FileAttributeEditor';
import type { FileMetadata } from './schemas';

vi.mock('./api', () => ({
  uploadFiles: vi.fn(),
  fileDownloadUrl: (id: string) => `/files/${id}`,
}));

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
});

describe('FileAttributeEditor', () => {
  it('queues at most one dropped file for a single-file attribute', () => {
    const single = {
      ...attribute,
      file_policy: { ...attribute.file_policy!, cardinality: 'one' as const },
    };
    const view = render(
      <FileAttributeEditor
        attribute={single}
        contextId={null}
        disabled={false}
        entityId={entityId}
        files={[]}
      />,
    );
    fireEvent.drop(view.container.querySelector('.MuiBox-root')!, {
      dataTransfer: {
        files: [file, new File(['more'], 'second.txt', { type: 'text/plain' })],
      },
    });
    expect(screen.getByText('document.txt')).toBeTruthy();
    expect(screen.queryByText('second.txt')).toBeNull();
    expect(
      (
        screen.getByRole('button', {
          name: 'Choose or drop files',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    fireEvent.drop(view.container.querySelector('.MuiBox-root')!, {
      dataTransfer: {
        files: [new File(['again'], 'third.txt', { type: 'text/plain' })],
      },
    });
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
    const view = renderEditor();
    fireEvent.drop(view.container.querySelector('.MuiBox-root')!, {
      dataTransfer: { files: [file] },
    });
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
});
