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
  it('says what the predicate checks in the active language', async () => {
    await i18n.changeLanguage('en');
    const attributes = [{ code: 'title', name: 'Listing title' }];
    const { rerender } = render(
      <RulePreview attributes={attributes} rule={rule} />,
    );
    expect(screen.getByText('Listing title has a value')).toBeTruthy();
    await i18n.changeLanguage('pl');
    rerender(<RulePreview attributes={attributes} rule={rule} />);
    expect(screen.getByText('Listing title ma wartość')).toBeTruthy();
  });

  it('spells out each part of a combined predicate and names events', async () => {
    await i18n.changeLanguage('en');
    render(
      <RulePreview
        attributes={[
          {
            code: 'condition',
            name: 'Condition',
            value_type: 'string',
            value_schema: {
              type: 'string',
              enum: ['new'],
              'x-attricat-enum-labels': { new: 'New' },
            },
          },
          { code: 'mileage_km', name: 'Mileage (km)' },
        ]}
        rule={{
          ...rule,
          triggers: [{ type: 'event', event_type: 'record.updated.v1' }],
          predicate: {
            type: predicateTypes.anyOf,
            predicates: [
              {
                type: predicateTypes.oneOf,
                attribute_code: 'condition',
                values: ['new'],
              },
              { type: predicateTypes.required, attribute_code: 'mileage_km' },
            ],
          },
        }}
      />,
    );
    expect(screen.getByText('At least one of these holds:')).toBeTruthy();
    expect(screen.getByText('Condition is one of "New"')).toBeTruthy();
    expect(screen.getByText('Mileage (km) has a value')).toBeTruthy();
    expect(screen.getByText('When a record is updated')).toBeTruthy();
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
