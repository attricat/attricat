import { useTranslation } from 'react-i18next';
import { UnsavedChangesGuard } from '../../drafts/UnsavedChangesGuard';

/** The record the Explorer shows in its panel, if the location has one. */
const panelRecord = (search: unknown) =>
  (search as { record?: unknown } | undefined)?.record;

/**
 * Asks before a navigation leaves the record while field changes that could
 * not be saved are still waiting. Render it only while there are such changes.
 */
export const UnsavedFieldChangesGuard = () => {
  const { t } = useTranslation();
  return (
    <UnsavedChangesGuard
      description={t('records.unsavedFieldChangesDescription')}
      shouldBlock={({ current, next }) =>
        next.pathname !== current.pathname ||
        panelRecord(next.search) !== panelRecord(current.search)
      }
      title={t('records.unsavedFieldChangesTitle')}
    />
  );
};
