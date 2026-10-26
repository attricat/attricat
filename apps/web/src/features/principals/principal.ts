import type { Attribute } from '../records/api';
import { principalKinds, PRINCIPAL_SCHEMA_KEY } from './constants';
import {
  principalConfigurationSchema,
  type Directory,
  type DirectoryTeam,
  type DirectoryUser,
  type PrincipalConfiguration,
  type PrincipalKind,
} from './schemas';
import { attributeValueTypes } from '../records/valueTypes';

const referencePattern =
  /^(user|team):([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/;

// Every string cell of a table asks for its column's configuration, so each
// attribute's configuration is parsed once.
const principalConfigurations = new WeakMap<
  Attribute,
  PrincipalConfiguration | undefined
>();

/** The assignment configuration of a string attribute, if it has one. */
export const principalConfiguration = (
  attribute: Attribute,
): PrincipalConfiguration | undefined => {
  if (
    attribute.value_type !== attributeValueTypes.string ||
    !attribute.value_schema ||
    typeof attribute.value_schema !== 'object'
  )
    return undefined;
  if (principalConfigurations.has(attribute))
    return principalConfigurations.get(attribute);
  const result = principalConfigurationSchema.safeParse(
    attribute.value_schema[PRINCIPAL_SCHEMA_KEY],
  );
  const configuration = result.success ? result.data : undefined;
  principalConfigurations.set(attribute, configuration);
  return configuration;
};

type PrincipalReference = { kind: PrincipalKind; id: string };

/** Parses the stored form, `user:<uuid>` or `team:<uuid>`. */
export const parsePrincipalReference = (
  value: unknown,
): PrincipalReference | undefined => {
  if (typeof value !== 'string') return undefined;
  const [, kind, id] = referencePattern.exec(value) ?? [];
  return (kind === principalKinds.user || kind === principalKinds.team) && id
    ? { kind, id }
    : undefined;
};

export const principalReference = (kind: PrincipalKind, id: string) =>
  `${kind}:${id}`;

export const userLabel = (user: DirectoryUser) =>
  user.display_name?.trim() || user.email;

type ResolvedPrincipal =
  | { kind: 'user'; user: DirectoryUser; label: string }
  | { kind: 'team'; team: DirectoryTeam; label: string };

/** Looks up a stored reference; `undefined` when the directory lacks it. */
export const resolvePrincipal = (
  directory: Directory | undefined,
  value: unknown,
): ResolvedPrincipal | undefined => {
  const reference = parsePrincipalReference(value);
  if (!reference || !directory) return undefined;
  if (reference.kind === principalKinds.user) {
    const user = directory.users.find((item) => item.id === reference.id);
    return user && { kind: principalKinds.user, user, label: userLabel(user) };
  }
  const team = directory.teams.find((item) => item.id === reference.id);
  return team && { kind: principalKinds.team, team, label: team.name };
};

/** Whether a directory entry can be newly assigned. */
export const isAssignable = (principal: ResolvedPrincipal) =>
  principal.kind === principalKinds.user
    ? principal.user.active
    : !principal.team.deleted;

type PrincipalOption = { value: string; principal: ResolvedPrincipal };

/**
 * Every directory entry of the configured kinds, including former members and
 * deleted teams (for example, to filter by past assignments).
 */
export const directoryOptions = (
  directory: Directory | undefined,
  config: PrincipalConfiguration,
): PrincipalOption[] => {
  if (!directory) return [];
  const users = config.kinds.includes(principalKinds.user)
    ? directory.users.map((user) => ({
        value: principalReference(principalKinds.user, user.id),
        principal: { kind: principalKinds.user, user, label: userLabel(user) },
      }))
    : [];
  const teams = config.kinds.includes(principalKinds.team)
    ? directory.teams.map((team) => ({
        value: principalReference(principalKinds.team, team.id),
        principal: { kind: principalKinds.team, team, label: team.name },
      }))
    : [];
  return [...users, ...teams];
};

/** Active users and existing teams of the configured kinds. */
export const assignableOptions = (
  directory: Directory | undefined,
  config: PrincipalConfiguration,
): PrincipalOption[] =>
  directoryOptions(directory, config).filter((option) =>
    isAssignable(option.principal),
  );
