import { ApiRequestError } from '../../api/request';
import i18n from '../../i18n';
import { getRecordsPublications, type RecordPublicationStatus } from './api';

/** Most records one publication lookup may name. */
const maxRecordsPerLookup = 100;

type Waiter = {
  resolve: (publications: RecordPublicationStatus[]) => void;
  reject: (error: unknown) => void;
};

let waiting = new Map<string, Waiter[]>();
let scheduled = false;

const lookUp = async (chunk: string[], batch: Map<string, Waiter[]>) => {
  try {
    const { items } = await getRecordsPublications(chunk);
    const found = new Map(
      items.map((item) => [item.record_id, item.publications]),
    );
    for (const recordId of chunk) {
      const publications = found.get(recordId);
      // The lookup leaves out records the caller cannot read or that no longer
      // exist; they fail as their own lookup would, without one request each.
      batch
        .get(recordId)
        ?.forEach((waiter) =>
          publications === undefined
            ? waiter.reject(
                new ApiRequestError(
                  404,
                  i18n.t('errors.recordUnavailable'),
                  'not_found',
                ),
              )
            : waiter.resolve(publications),
        );
    }
  } catch (error) {
    for (const recordId of chunk)
      batch.get(recordId)?.forEach((waiter) => waiter.reject(error));
  }
};

const flush = async () => {
  const batch = waiting;
  waiting = new Map();
  scheduled = false;
  const recordIds = [...batch.keys()];
  const chunks: string[][] = [];
  for (let start = 0; start < recordIds.length; start += maxRecordsPerLookup)
    chunks.push(recordIds.slice(start, start + maxRecordsPerLookup));
  await Promise.all(chunks.map((chunk) => lookUp(chunk, batch)));
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
