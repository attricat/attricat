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
