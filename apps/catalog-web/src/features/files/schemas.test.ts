import { describe, expect, it } from 'vitest';
import { fileMetadataSchema, fileUploadResultSchema } from './schemas';

const id = '123e4567-e89b-12d3-a456-426614174000';

describe('file schemas', () => {
  it('parses metadata without exposing object-storage keys', () => {
    expect(
      fileMetadataSchema.parse({
        id,
        filename: 'manual.pdf',
        mime_type: 'application/pdf',
        byte_size: 1024,
        sha256: 'abc',
        status: 'ready',
        variants: [],
      }),
    ).toMatchObject({ filename: 'manual.pdf', status: 'ready' });
  });

  it('hydrates upload results with an empty variants list', () => {
    expect(
      fileUploadResultSchema.parse({
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
      }).files[0].variants,
    ).toEqual([]);
  });
});
