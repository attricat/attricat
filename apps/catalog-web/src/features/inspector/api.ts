import { requestNoContent } from '../../api/request';

export const getApiHealth = async () => {
  await requestNoContent('/api/health');
  return true;
};
