import { describe, expect, it } from 'vitest';
import type { Attribute } from '../entities/api';
import {
  assignableOptions,
  parsePrincipalReference,
  principalConfiguration,
  resolvePrincipal,
} from './principal';
import type { Directory } from './schemas';

const userId = '6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60';
const formerId = '7b2f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f61';
const teamId = '8c3f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f62';
const deletedTeamId = '9d4f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f63';

const directory: Directory = {
  users: [
    {
      id: userId,
      display_name: 'Ada',
      email: 'ada@example.test',
      active: true,
    },
    {
      id: formerId,
      display_name: null,
      email: 'former@example.test',
      active: false,
    },
  ],
  teams: [
    { id: teamId, code: 'qa', name: 'Quality', deleted: false },
    { id: deletedTeamId, code: 'old', name: 'Old team', deleted: true },
  ],
};

const assignee = (kinds: string[]): Attribute => ({
  code: 'assignee',
  value_type: 'string',
  value_schema: {
    type: 'string',
    'x-attricat-principal': { version: 1, kinds },
  },
});

describe('user-or-team assignments', () => {
  it('reads the annotation on string attributes only', () => {
    expect(principalConfiguration(assignee(['user']))?.kinds).toEqual(['user']);
    expect(principalConfiguration(assignee(['group']))).toBeUndefined();
    expect(
      principalConfiguration({ ...assignee(['user']), value_type: 'json' }),
    ).toBeUndefined();
  });

  it('parses only canonical references', () => {
    expect(parsePrincipalReference(`user:${userId}`)).toEqual({
      kind: 'user',
      id: userId,
    });
    expect(parsePrincipalReference(`USER:${userId}`)).toBeUndefined();
    expect(parsePrincipalReference(userId)).toBeUndefined();
    expect(parsePrincipalReference(1)).toBeUndefined();
  });

  it('resolves names, falling back to email', () => {
    expect(resolvePrincipal(directory, `user:${userId}`)?.label).toBe('Ada');
    expect(resolvePrincipal(directory, `user:${formerId}`)?.label).toBe(
      'former@example.test',
    );
    expect(resolvePrincipal(directory, `team:${teamId}`)?.label).toBe(
      'Quality',
    );
    expect(resolvePrincipal(directory, `team:${userId}`)).toBeUndefined();
  });

  it('offers only active users and existing teams of the allowed kinds', () => {
    const config = principalConfiguration(assignee(['user', 'team']))!;
    expect(
      assignableOptions(directory, config).map((option) => option.value),
    ).toEqual([`user:${userId}`, `team:${teamId}`]);
    const usersOnly = principalConfiguration(assignee(['user']))!;
    expect(
      assignableOptions(directory, usersOnly).map((option) => option.value),
    ).toEqual([`user:${userId}`]);
  });
});
