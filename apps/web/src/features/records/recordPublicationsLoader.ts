import {
  getRecordPublications,
  getRecordsPublications,
  type RecordPublicationStatus,
} from './api';

/** Most records one publication lookup may name. */
const maxRecordsPerLookup = 100;

type Waiter = {
  resolve: (publications: RecordPublicationStatus[]) => void;
  reject: (error: unknown) => void;
};

let waiting = new Map<string, Waiter[]>();
let scheduled = false;

const flush = async () => {
  const batch = waiting;
  waiting = new Map();
  scheduled = false;
  const recordIds = [...batch.keys()];
  for (let start = 0; start < recordIds.length; start += maxRecordsPerLookup) {
    const chunk = recordIds.slice(start, start + maxRecordsPerLookup);
    try {
      const { items } = await getRecordsPublications(chunk);
      const found = new Map(
        items.map((item) => [item.record_id, item.publications]),
      );
      for (const recordId of chunk) {
        const publications = found.get(recordId);
        // A left-out record fails as its own lookup would.
        const result =
          publications === undefined
            ? getRecordPublications(recordId)
            : Promise.resolve(publications);
        result.then(
          (value) =>
            batch.get(recordId)?.forEach((waiter) => waiter.resolve(value)),
          (error: unknown) =>
            batch.get(recordId)?.forEach((waiter) => waiter.reject(error)),
        );
      }
    } catch (error) {
      for (const recordId of chunk)
        batch.get(recordId)?.forEach((waiter) => waiter.reject(error));
    }
  }
};

/**
 * A record's channel publication statuses. Lookups started in the same task,
 * such as one per visible table row, share one request.
 */
export const loadRecordPublications = (recordId: string) =>
  new Promise<RecordPublicationStatus[]>((resolve, reject) => {
    waiting.set(recordId, [
      ...(waiting.get(recordId) ?? []),
      { resolve, reject },
    ]);
    if (!scheduled) {
      scheduled = true;
      setTimeout(() => void flush(), 0);
    }
  });
