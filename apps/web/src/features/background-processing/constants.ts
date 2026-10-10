const millisecondsPerSecond = 1_000;

export const backgroundProcessingRefreshInterval = 30_000;
export const backgroundProcessingRefreshSeconds =
  backgroundProcessingRefreshInterval / millisecondsPerSecond;

export const taskKindTranslationKeys: Record<string, string> = {
  'agent_run.v1': 'backgroundProcessing.kinds.agent',
  'event_delivery.v1': 'backgroundProcessing.kinds.event',
  'workflow_run.v1': 'backgroundProcessing.kinds.workflow',
  'rule_run.v1': 'backgroundProcessing.kinds.rule',
  'blueprint_migration_batch.v1': 'backgroundProcessing.kinds.migration',
  'extension_operation_run.v1': 'backgroundProcessing.kinds.extension',
};

export const otherTaskKindTranslationKey = 'backgroundProcessing.kinds.other';

/** Header translation keys for the numeric columns, in display order. */
export const backgroundProcessingColumnKeys = [
  'backgroundProcessing.queued',
  'backgroundProcessing.running',
  'backgroundProcessing.failed',
  'backgroundProcessing.expired',
  'backgroundProcessing.oldestDue',
] as const;

/** Displayed when no queued task is due yet. */
export const noDueTaskPlaceholder = '—';
