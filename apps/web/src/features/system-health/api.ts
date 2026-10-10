import { request } from '../../api/request';
import { systemHealthSchema } from './schemas';

export const getSystemHealth = () =>
  request('/api/system/health', systemHealthSchema);
