import { useBlocker, type ShouldBlockFn } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { DiscardChangesDialog } from './DiscardChangesDialog';

const leavesPage: ShouldBlockFn = ({ current, next }) =>
  next.pathname !== current.pathname;

/**
 * Asks before an in-app navigation drops changes that are not saved. Render
 * it only while there are such changes; leaving the browser page is warned
 * about separately.
 */
export const UnsavedChangesGuard = ({
  title,
  description,
  shouldBlock = leavesPage,
  onDiscard,
}: {
  title: string;
  description: string;
  /** Which navigations leave the changes behind; by default, another page. */
  shouldBlock?: ShouldBlockFn;
  /** Runs when the user chooses to leave, before the navigation continues. */
  onDiscard?: () => void;
}) => {
  const { t } = useTranslation();
  const blocker = useBlocker({
    shouldBlockFn: shouldBlock,
    enableBeforeUnload: false,
    withResolver: true,
  });
  return (
    <DiscardChangesDialog
      description={description}
      discardLabel={t('drafts.discardAndLeave')}
      onDiscard={() => {
        onDiscard?.();
        blocker.proceed?.();
      }}
      onKeep={() => blocker.reset?.()}
      open={blocker.status === 'blocked'}
      title={title}
    />
  );
};
