// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import type { RecordItem } from '../records/api';
import {
  addPickerCreatedRecord,
  forgetPickerCreatedRecords,
  readPickerCreatedRecords,
  readPickerSelection,
  writePickerSelection,
} from './relationshipPickerStorage';

const token = '4f0c8e3c-9b0d-4f5a-8f6e-2a1b3c4d5e6f';
const first = '8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f01';
const second = '8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f02';
const item = {
  id: first,
  blueprint_version: 1,
  schema_outdated: false,
  system_tags: [],
  is_sample: false,
  display: { default: 'Bolt' },
  preview: {},
  table_values: {},
} as unknown as RecordItem;

describe('relationshipPickerStorage', () => {
  afterEach(() => window.sessionStorage.clear());

  it('keeps a picker tab selection per token and ignores damaged data', () => {
    expect(readPickerSelection(token)).toEqual([]);
    writePickerSelection(token, [item]);
    expect(readPickerSelection(token).map((record) => record.id)).toEqual([
      first,
    ]);
    expect(readPickerSelection('another-picker')).toEqual([]);

    window.sessionStorage.setItem(
      `attricat.relationshipPicker.${token}.selection`,
      '{not json',
    );
    expect(readPickerSelection(token)).toEqual([]);
    window.sessionStorage.setItem(
      `attricat.relationshipPicker.${token}.selection`,
      JSON.stringify([{ id: 'not-a-record' }]),
    );
    expect(readPickerSelection(token)).toEqual([]);
  });

  it('remembers created records until the Explorer adds them', () => {
    addPickerCreatedRecord(token, first);
    addPickerCreatedRecord(token, first);
    addPickerCreatedRecord(token, second);
    expect(readPickerCreatedRecords(token)).toEqual([first, second]);
    // Reading does not forget, so an interrupted lookup can retry.
    expect(readPickerCreatedRecords(token)).toEqual([first, second]);

    forgetPickerCreatedRecords(token, [first]);
    expect(readPickerCreatedRecords(token)).toEqual([second]);
    forgetPickerCreatedRecords(token, [second]);
    expect(readPickerCreatedRecords(token)).toEqual([]);
    expect(
      window.sessionStorage.getItem(
        `attricat.relationshipPicker.${token}.created`,
      ),
    ).toBeNull();
  });
});
