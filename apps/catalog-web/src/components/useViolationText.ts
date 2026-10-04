import { useTranslation } from 'react-i18next';
import type { CheckViolation } from '../api/checkViolations';

/** One violation's message, naming the contexts in which it failed. */
export const useViolationText = () => {
  const { t } = useTranslation();
  return (violation: CheckViolation) =>
    violation.contexts.length > 0
      ? t('entities.checks.violationInContexts', {
          message: violation.message,
          contexts: violation.contexts.join(', '),
        })
      : violation.message;
};
