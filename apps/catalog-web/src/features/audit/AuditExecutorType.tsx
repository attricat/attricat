import { useTranslation } from 'react-i18next';
import { IconLabel } from '../../components/IconLabel';
import { AgentIcon, AssignedUserIcon } from '../../components/systemIcons';
import type { AuditEvent } from './api';
import { executorTypes } from './constants';

/** Whether a person or an agent performed an action, with its icon. */
export const AuditExecutorType = ({
  executorType,
}: {
  executorType: AuditEvent['executor_type'];
}) => {
  const { t } = useTranslation();
  return executorType === executorTypes.agent ? (
    <IconLabel icon={AgentIcon}>{t('audit.agent')}</IconLabel>
  ) : (
    <IconLabel icon={AssignedUserIcon}>{t('audit.human')}</IconLabel>
  );
};
