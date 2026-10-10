import i18n from 'i18next';

export const parseFutureExpiry = (value: string) => {
  const expiresAt = new Date(value);
  if (Number.isNaN(expiresAt.getTime()) || expiresAt <= new Date()) {
    throw new Error(i18n.t('workspace.invitationExpiry'));
  }
  return expiresAt;
};
