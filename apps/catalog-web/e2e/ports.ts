const port = (name: 'CATALOG_E2E_API_PORT' | 'CATALOG_E2E_WEB_PORT') => {
  const value = process.env[name];
  if (!value) throw new Error(`${name} must be set to run E2E tests`);
  return value;
};

export const e2eApiPort = port('CATALOG_E2E_API_PORT');
export const e2eWebPort = port('CATALOG_E2E_WEB_PORT');
export const e2eApiUrl = `http://127.0.0.1:${e2eApiPort}`;
export const e2eWebUrl = `http://127.0.0.1:${e2eWebPort}`;
