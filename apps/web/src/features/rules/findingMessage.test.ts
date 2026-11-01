import { afterEach, describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import { findingMessage, type FindingAttribute } from './findingMessage';

const attributes: FindingAttribute[] = [
  {
    code: 'listing_status',
    name: 'Listing review',
    value_type: 'string',
    value_schema: {
      type: 'string',
      enum: ['draft', 'approved'],
      'x-attricat-status': {
        version: 1,
        transitions: [],
        options: [
          { code: 'draft', label: 'Draft' },
          { code: 'approved', label: 'Approved' },
        ],
      },
    },
  },
  {
    code: 'condition',
    name: 'Condition',
    value_type: 'string',
    value_schema: {
      type: 'string',
      enum: ['new', 'used'],
      'x-attricat-enum-labels': { new: 'New', used: 'Used' },
    },
  },
  { code: 'reserved_until', name: 'Reserved until' },
];

describe('findingMessage', () => {
  afterEach(() => i18n.changeLanguage('en'));

  it('words the messages the server sends in the user language', async () => {
    await i18n.changeLanguage('pl');
    expect(
      findingMessage(
        `None of the alternatives hold: 'condition' must be one of "new"; or 'reserved_until' is required`,
        attributes,
      ),
    ).toBe(
      'Żaden z wariantów nie jest spełniony: Pole Condition musi mieć jedną z wartości: "New"; lub Pole Reserved until jest wymagane',
    );
    expect(
      findingMessage(`Another record has the same 'condition'`, attributes),
    ).toBe('Inny rekord ma taką samą wartość: Condition');
    expect(
      findingMessage(
        `'reserved_until' must be greater than 30 days from now`,
        attributes,
      ),
    ).toBe('Pole Reserved until musi być większe niż za 30 dni');
    expect(findingMessage(`'x' was seen by a future server`, [])).toBe(
      `'x' was seen by a future server`,
    );
  });

  it('names attributes and their options by their labels', () => {
    expect(
      findingMessage(`'listing_status' must be one of "approved"`, attributes),
    ).toBe('Listing review must be one of "Approved"');
  });

  it('labels every attribute of a combined message', () => {
    expect(
      findingMessage(
        `None of the alternatives hold: 'condition' must be one of "new"; or 'reserved_until' is required`,
        attributes,
      ),
    ).toBe(
      'None of the alternatives hold: Condition must be one of "New"; or Reserved until is required',
    );
  });

  it('keeps codes and values it does not know', () => {
    expect(
      findingMessage(
        `'price' must be one of "cheap"; 'condition' must be one of "mint"`,
        attributes,
      ),
    ).toBe(`'price' must be one of "cheap"; Condition must be one of "mint"`);
  });
});
