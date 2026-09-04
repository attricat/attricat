export const getApiHealth = async () => {
  const response = await fetch('/api/health');
  if (!response.ok) throw new Error('API health check failed');
  return true;
};
