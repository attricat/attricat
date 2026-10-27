// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { DefinitionTemplateDialog } from './DefinitionTemplateDialog';

describe('DefinitionTemplateDialog', () => {
  it('tells assistive technology which template is selected', () => {
    render(
      <DefinitionTemplateDialog
        onClose={vi.fn()}
        onSelect={vi.fn()}
        open
        selectedIndex={1}
        templates={[
          { definition: '', descriptionKey: 'First', labelKey: 'Blank' },
          { definition: '', descriptionKey: 'Second', labelKey: 'Product' },
        ]}
      />,
    );
    expect(
      screen
        .getByRole('button', { name: /Product/ })
        .getAttribute('aria-pressed'),
    ).toBe('true');
    expect(
      screen
        .getByRole('button', { name: /Blank/ })
        .getAttribute('aria-pressed'),
    ).toBe('false');
  });
});
