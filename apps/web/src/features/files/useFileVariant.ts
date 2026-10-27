import { useQuery } from '@tanstack/react-query';
import { fileStatuses } from './constants';
import { fileMetadataOptions } from './queryOptions';

/**
 * A file's metadata and one of its rendered variants. The variant is
 * unavailable once the file failed, was deleted, finished without it, or its
 * metadata could not be loaded; until then it may still be processing.
 */
export const useFileVariant = (fileId: string, kind: string) => {
  const metadata = useQuery(fileMetadataOptions(fileId));
  const status = metadata.data?.status;
  const variant = metadata.data?.variants.find((item) => item.kind === kind);
  const ready = status === fileStatuses.ready;
  const unavailable =
    metadata.isError ||
    (ready && !variant) ||
    status === fileStatuses.failed ||
    status === fileStatuses.deleted;
  return { metadata, ready, unavailable, variant };
};
