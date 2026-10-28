// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import en from '../../i18n/locales/en.json';
import pl from '../../i18n/locales/pl.json';
import type { CompiledRule } from './api';
import { predicateTypes } from './constants';
import { RulePreview } from './RulePreview';

const rule: CompiledRule = {
  code: 'title-required',
  name: 'Records have a title',
  severity: 'warning',
  triggers: [{ type: 'manual' }],
  predicate: { type: predicateTypes.required, attribute_code: 'title' },
};

afterEach(() => i18n.changeLanguage('en'));

describe('RulePreview', () => {
  it('names the predicate type in the active language', async () => {
    await i18n.changeLanguage('en');
    const { rerender } = render(<RulePreview rule={rule} />);
    expect(screen.getByText('Required value on title')).toBeTruthy();
    await i18n.changeLanguage('pl');
    rerender(<RulePreview rule={rule} />);
    expect(screen.getByText('Wymagana wartość dla title')).toBeTruthy();
  });

  it('shows the description with its line breaks', async () => {
    await i18n.changeLanguage('en');
    render(
      <RulePreview
        rule={{ ...rule, description: 'Shoppers search by title.\nKeep it.' }}
      />,
    );
    const description = screen.getByText(/Shoppers search by title/);
    expect(description.textContent).toBe('Shoppers search by title.\nKeep it.');
  });

  it('keeps the code of a predicate type it does not know', async () => {
    await i18n.changeLanguage('en');
    render(<RulePreview rule={{ ...rule, predicate: { type: 'novel' } }} />);
    expect(screen.getByText('novel')).toBeTruthy();
  });

  it('translates every predicate type in both languages', () => {
    for (const locale of [en, pl])
      expect(Object.keys(locale.rules.preview.predicateTypes).sort()).toEqual(
        Object.values(predicateTypes).sort(),
      );
  });
});
