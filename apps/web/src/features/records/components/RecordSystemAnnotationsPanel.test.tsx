// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../../i18n';
import { tagColorSwatches } from '../../../app/tagColors';
import { TagColorsTestProvider } from '../../tag-colors/TagColorsTestProvider';
import { hexToRgb } from '../../tag-colors/testColors';
import { RecordSystemAnnotationsPanel } from './RecordSystemAnnotationsPanel';

describe('RecordSystemAnnotationsPanel', () => {
  it('shows tags without the sample marker, and the metadata', () => {
    render(
      <RecordSystemAnnotationsPanel
        metadata={{ review: { reason: 'price changed' } }}
        tags={['attricat.sample', 'needs-review']}
      />,
      { wrapper: TagColorsTestProvider },
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
      { wrapper: TagColorsTestProvider },
    );

    expect(screen.getByText('No system tags or metadata')).toBeTruthy();
  });

  it('shows each tag in its workspace color', () => {
    render(
      <TagColorsTestProvider
        colors={{
          defaults: { 'attricat.needs_attention': 'yellow' },
          overrides: { reviewed: 'green' },
        }}
      >
        <RecordSystemAnnotationsPanel
          metadata={{}}
          tags={['attricat.needs_attention', 'reviewed', 'plain']}
        />
      </TagColorsTestProvider>,
    );

    const fill = (label: string) =>
      getComputedStyle(screen.getByText(label).closest('.MuiChip-root')!)
        .backgroundColor;
    const { light } = tagColorSwatches;
    expect(fill('Needs attention')).toBe(hexToRgb(light.yellow.background));
    expect(fill('reviewed')).toBe(hexToRgb(light.green.background));
    expect(fill('plain')).not.toBe(hexToRgb(light.green.background));
  });
});
