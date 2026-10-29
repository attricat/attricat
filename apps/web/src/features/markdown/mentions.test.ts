import { describe, expect, it } from 'vitest';
import {
  activeMentionQuery,
  insertMention,
  mentionedUserId,
  mentionSuggestions,
} from './mentions';

const ada = {
  id: '6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60',
  display_name: 'Ada Lovelace',
  email: 'ada@example.test',
  active: true,
};
const bob = {
  id: '7b2f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f61',
  display_name: null,
  email: 'bob@example.test',
  active: true,
};
const former = {
  id: '8c3f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f62',
  display_name: 'Adam Former',
  email: 'adam@example.test',
  active: false,
};

describe('mentions', () => {
  it('finds the @ being typed before the caret', () => {
    expect(activeMentionQuery('@', 1)).toEqual({ start: 0, query: '' });
    expect(activeMentionQuery('Hi @Ada Lo', 10)).toEqual({
      start: 3,
      query: 'Ada Lo',
    });
    expect(activeMentionQuery('Hi @Ada and more', 7)).toEqual({
      start: 3,
      query: 'Ada',
    });
    expect(activeMentionQuery('mail@example.test', 17)).toBeUndefined();
    expect(activeMentionQuery('@Ada\nnext', 9)).toBeUndefined();
    expect(activeMentionQuery(`@${'a'.repeat(51)}`, 52)).toBeUndefined();
  });

  it('suggests active users by name or email, prefix matches first', () => {
    expect(mentionSuggestions([bob, ada, former], 'ad', 8)).toEqual([ada]);
    expect(mentionSuggestions([ada, bob], 'example', 8)).toEqual([ada, bob]);
    expect(mentionSuggestions([bob, ada], 'a', 8)).toEqual([ada, bob]);
    expect(mentionSuggestions([ada, bob], '', 1)).toEqual([ada]);
  });

  it('inserts an escaped mention link and moves the caret past it', () => {
    const text = 'Hi @ad, see this';
    const result = insertMention(
      text,
      { start: 3, query: 'ad' },
      { ...ada, display_name: 'Ada [QA]_lead' },
    );
    const link = `[@Ada \\[QA\\]\\_lead](user:${ada.id}) `;
    expect(result).toEqual({
      text: `Hi ${link}, see this`,
      cursor: 3 + link.length,
    });
  });

  it('recognises only user references as mentions', () => {
    expect(mentionedUserId(`user:${ada.id}`)).toBe(ada.id);
    expect(mentionedUserId(`team:${ada.id}`)).toBeUndefined();
    expect(mentionedUserId('user:not-a-uuid')).toBeUndefined();
    expect(mentionedUserId('https://example.test')).toBeUndefined();
    expect(mentionedUserId(undefined)).toBeUndefined();
  });
});
