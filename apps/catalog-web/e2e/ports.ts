const port = (
  name:
    | 'CATALOG_E2E_API_PORT'
    | 'CATALOG_E2E_WEB_PORT'
    | 'CATALOG_E2E_MAILPIT_SMTP_PORT'
    | 'CATALOG_E2E_MAILPIT_UI_PORT'
    | 'CATALOG_E2E_S3_PORT',
) => {
  const value = process.env[name];
  if (!value) throw new Error(`${name} must be set to run E2E tests`);
  return value;
};

export const e2eApiPort = port('CATALOG_E2E_API_PORT');
export const e2eWebPort = port('CATALOG_E2E_WEB_PORT');
export const e2eMailpitSmtpPort = port('CATALOG_E2E_MAILPIT_SMTP_PORT');
export const e2eMailpitUiPort = port('CATALOG_E2E_MAILPIT_UI_PORT');
export const e2eS3Port = port('CATALOG_E2E_S3_PORT');
export const e2eApiUrl = `http://127.0.0.1:${e2eApiPort}`;
export const e2eWebUrl = `http://127.0.0.1:${e2eWebPort}`;
export const e2eMailpitUrl = `http://127.0.0.1:${e2eMailpitUiPort}`;
export const e2eS3Url = `http://127.0.0.1:${e2eS3Port}`;
