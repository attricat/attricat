import type { ExportStatus } from './constants';
import type { ExportStatusCount } from './schemas';

/** The select value standing for the default connection. */
export const allConnections = '';

export type ConnectionCounts = {
  connection: string | null;
  counts: Partial<Record<ExportStatus, number>>;
};

/** Status counts by connection, in a stable order. */
export const countsByConnection = (
  counts: ExportStatusCount[],
): ConnectionCounts[] => {
  const connections = [
    ...new Set(counts.map((count) => count.connection ?? allConnections)),
  ].sort();
  return connections.map((connection) => ({
    connection: connection === allConnections ? null : connection,
    counts: Object.fromEntries(
      counts
        .filter((count) => (count.connection ?? allConnections) === connection)
        .map((count) => [count.status, count.count]),
    ),
  }));
};
