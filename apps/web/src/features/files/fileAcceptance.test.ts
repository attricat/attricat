import { describe, expect, it } from 'vitest';
import type { Attribute } from '../records/api';
import { fileRejection, permittedFileTypes } from './fileAcceptance';

const filePolicy = {
  cardinality: 'many' as const,
  ordered: false,
  allowed_mime_groups: [],
  allowed_extensions: ['.pdf'],
  max_bytes: 10,
  purposes: [],
  image_only: false,
};
const attribute = { file_policy: filePolicy } as unknown as Attribute;
const file = (name: string, size: number, type: string) =>
  new File(['x'.repeat(size)], name, { type });

describe('fileRejection', () => {
  it('names a file too large before its type', () => {
    expect(fileRejection(file('big.png', 11, 'image/png'), attribute)).toBe(
      'size',
    );
  });

  it('refuses a type the field does not permit', () => {
    expect(fileRejection(file('red.png', 3, 'image/png'), attribute)).toBe(
      'type',
    );
    expect(
      fileRejection(file('form.pdf', 3, 'application/pdf'), attribute),
    ).toBeUndefined();
  });
});

describe('permittedFileTypes', () => {
  it('lists extensions, or the supported images of an image field', () => {
    expect(permittedFileTypes(filePolicy)).toBe('PDF');
    expect(
      permittedFileTypes({
        ...filePolicy,
        allowed_extensions: [],
        image_only: true,
      }),
    ).toMatch(/^PNG|JPEG/);
  });
});
