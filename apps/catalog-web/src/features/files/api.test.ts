import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiRequestError } from '../entities/api';
import { fileDownloadUrl, uploadFiles } from './api';

const id = '123e4567-e89b-12d3-a456-426614174000';
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => fetchMock.mockReset());

describe('file API client', () => {
  it('uploads multipart files and reports completion progress', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          attribute_code: 'images',
          context_id: id,
          files: [
            {
              id,
              filename: 'shirt.png',
              mime_type: 'image/png',
              byte_size: 42,
              sha256: 'abc',
              status: 'queued',
            },
          ],
        }),
    });
    const progress = vi.fn();
    const result = await uploadFiles({
      entityId: id,
      attributeCode: 'images',
      contextId: id,
      files: [new File(['image'], 'shirt.png', { type: 'image/png' })],
      onProgress: progress,
    });

    expect(fetchMock.mock.calls[0][0]).toBe(
      `/api/entities/${id}/file-attributes/images/uploads`,
    );
    expect(result.files[0].filename).toBe('shirt.png');
    expect(progress).toHaveBeenCalledWith(100);
  });

  it('preserves structured upload errors and creates safe download paths', async () => {
    fetchMock.mockResolvedValue({
      ok: false,
      status: 422,
      json: () =>
        Promise.resolve({
          error: {
            code: 'unsupported_media_type',
            message: 'Unsupported file',
          },
        }),
    });

    await expect(
      uploadFiles({ entityId: id, attributeCode: 'manual', files: [] }),
    ).rejects.toBeInstanceOf(ApiRequestError);
    expect(fileDownloadUrl(id, 'thumbnail')).toBe(
      `/api/files/${id}/variants/thumbnail/download`,
    );
  });
});
