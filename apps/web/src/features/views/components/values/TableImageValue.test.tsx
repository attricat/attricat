// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../../../i18n';
import { TableImageValue } from './TableImageValue';

const fileId = '11111111-1111-4111-8111-111111111111';

const thumbnailVariant = {
  kind: 'thumbnail',
  mime_type: 'image/webp',
  width: 48,
  height: 48,
  byte_size: 12,
  sha256: 'def',
};

const image = (status: 'ready' | 'failed', variants = [thumbnailVariant]) => [
  {
    id: fileId,
    filename: 'main.png',
    mime_type: 'image/png',
    byte_size: 42,
    sha256: 'abc',
    status,
    variants,
  },
];

describe('TableImageValue', () => {
  it('uses the thumbnail download URL for a ready image', () => {
    const { container } = render(<TableImageValue value={image('ready')[0]} />);

    expect(container.querySelector('img')?.getAttribute('src')).toBe(
      `/api/files/${fileId}/variants/thumbnail/download`,
    );
  });

  it('never loads the original when a ready image has no thumbnail', () => {
    const { container } = render(
      <TableImageValue value={image('ready', [])} />,
    );

    expect(container.querySelector('img')).toBeNull();
    expect(screen.getByText('Thumbnail unavailable')).toBeTruthy();
  });

  it('shows the normal unset state when no file is assigned', () => {
    render(<TableImageValue value={[]} />);

    expect(screen.getByText('Not set')).toBeTruthy();
  });

  it('shows the unavailable status for a failed image', () => {
    render(<TableImageValue value={image('failed')} />);

    expect(screen.getByText('Thumbnail unavailable')).toBeTruthy();
  });
});
