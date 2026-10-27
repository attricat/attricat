// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../../i18n';
import { RecordSystemAnnotationsPanel } from './RecordSystemAnnotationsPanel';

describe('RecordSystemAnnotationsPanel', () => {
  it('shows tags without the sample marker, and the metadata', () => {
    render(
      <RecordSystemAnnotationsPanel
        metadata={{ review: { reason: 'price changed' } }}
        tags={['attricat.sample', 'needs-review']}
      />,
    );

    expect(screen.getByText('needs-review')).toBeTruthy();
    expect(screen.queryByText('attricat.sample')).toBeNull();
    expect(
      screen.getByRole('region', { name: 'System metadata' }).textContent,
    ).toContain('price changed');
  });

  it('explains where annotations come from when a record has none', () => {
    render(
      <RecordSystemAnnotationsPanel metadata={{}} tags={['attricat.sample']} />,
    );

    expect(screen.getByText('No system tags or metadata')).toBeTruthy();
  });
});
