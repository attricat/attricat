import { z } from 'zod';
import { request, requestNoContent, jsonRequest } from '../../api/request';
import { DIRECTORY_PATH, TEAMS_PATH } from './constants';
import { directorySchema, teamSchema } from './schemas';
import { uuidSegment } from '../../api/pathSegments';

const teamPath = (id: string) => `${TEAMS_PATH}/${uuidSegment(id)}`;

export type TeamInput = {
  code: string;
  name: string;
  member_user_ids: string[];
};

export const getDirectory = (signal?: AbortSignal) =>
  request(DIRECTORY_PATH, directorySchema, { signal });
export const listTeams = () => request(TEAMS_PATH, z.array(teamSchema));
export const createTeam = (input: TeamInput) =>
  request(TEAMS_PATH, teamSchema, jsonRequest('POST', input));
export const updateTeam = (
  id: string,
  input: Partial<Omit<TeamInput, 'code'>>,
) => request(teamPath(id), teamSchema, jsonRequest('PATCH', input));
export const deleteTeam = (id: string) =>
  requestNoContent(teamPath(id), { method: 'DELETE' });
