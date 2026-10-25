import { findingSeverities } from './constants';

const errorSeverities = new Set<string>([
  findingSeverities.error,
  findingSeverities.critical,
]);

/** The chip colour of a finding's severity, the same on every screen. */
export const findingSeverityColor = (severity: string) =>
  errorSeverities.has(severity)
    ? 'error'
    : severity === findingSeverities.info
      ? 'info'
      : 'warning';
