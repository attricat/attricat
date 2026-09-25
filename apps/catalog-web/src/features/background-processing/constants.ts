export const backgroundProcessingRefreshInterval = 30_000;

export const taskKindTranslationKeys: Record<string, string> = {
  'agent_run.v1': 'backgroundProcessing.kinds.agent',
  'event_delivery.v1': 'backgroundProcessing.kinds.event',
  'workflow_run.v1': 'backgroundProcessing.kinds.workflow',
  'rule_run.v1': 'backgroundProcessing.kinds.rule',
  'blueprint_migration_batch.v1': 'backgroundProcessing.kinds.migration',
  'extension_operation_run.v1': 'backgroundProcessing.kinds.extension',
};
