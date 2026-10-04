import { useTranslation } from 'react-i18next';
import { violationsText } from './checkViolations';
import { statusTransitionDenialCodes } from './constants';
import type { StatusTransitionDenial } from './status';

/**
 * Explains a refused status transition. The form error and the status
 * selector use the same text for the same denial.
 */
export const useStatusTransitionDenialText = () => {
  const { t } = useTranslation();
  return (denial: StatusTransitionDenial) => {
    if (denial.kind === 'undeclared')
      return t('entities.statusTransitionDenied');
    switch (denial.access.denial_code) {
      case statusTransitionDenialCodes.conditionsUnmet:
        return t('entities.statusConditionsUnmet', {
          conditions: violationsText(denial.access.unmet ?? []),
        });
      case statusTransitionDenialCodes.separationOfDuties:
        return t('entities.statusSeparationOfDuties');
      default:
        return t('entities.statusTransitionNotPermitted');
    }
  };
};
