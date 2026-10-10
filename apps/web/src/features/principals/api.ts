import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import { DIRECTORY_PATH, TEAMS_PATH } from './constants';
import { directorySchema, teamSchema } from './schemas';

const json = (method: string, body: unknown): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body),
});
const teamPath = (id: string) => `${TEAMS_PATH}/${z.uuid().parse(id)}`;

export type TeamInput = {
  code: string;
  name: string;
  member_user_ids: string[];
};

export const getDirectory = (signal?: AbortSignal) =>
  request(DIRECTORY_PATH, directorySchema, { signal });
export const listTeams = () => request(TEAMS_PATH, z.array(teamSchema));
export const createTeam = (input: TeamInput) =>
  request(TEAMS_PATH, teamSchema, json('POST', input));
export const updateTeam = (
  id: string,
  input: Partial<Omit<TeamInput, 'code'>>,
) => request(teamPath(id), teamSchema, json('PATCH', input));
export const deleteTeam = (id: string) =>
  requestNoContent(teamPath(id), { method: 'DELETE' });
