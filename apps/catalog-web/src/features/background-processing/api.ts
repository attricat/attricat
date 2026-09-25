import { request } from '../../api/request';
import { backgroundProcessingStatusSchema } from './schemas';

export const getBackgroundProcessingStatus = () =>
  request(
    '/api/data-health/background-processing',
    backgroundProcessingStatusSchema,
  );
