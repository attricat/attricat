import { FileIcon, FilesIcon, ImageIcon, ImagesIcon } from 'lucide-react';
import { describe, expect, it } from 'vitest';
import { fileAttributeIcon } from './fileAttributeIcon';

const policy = (cardinality: 'one' | 'many', image_only: boolean) => ({
  file_policy: {
    cardinality,
    ordered: false,
    allowed_mime_groups: [],
    allowed_extensions: [],
    purposes: [],
    image_only,
  },
});

describe('fileAttributeIcon', () => {
  it('tells images from other files, and one from several', () => {
    expect(fileAttributeIcon(policy('one', true))).toBe(ImageIcon);
    expect(fileAttributeIcon(policy('many', true))).toBe(ImagesIcon);
    expect(fileAttributeIcon(policy('one', false))).toBe(FileIcon);
    expect(fileAttributeIcon(policy('many', false))).toBe(FilesIcon);
    expect(fileAttributeIcon({})).toBe(FileIcon);
  });
});
