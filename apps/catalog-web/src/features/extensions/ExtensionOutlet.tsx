import { Alert, Stack } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { ExtensionFrame } from './ExtensionFrame';
import { getExtensionRuntime } from './api';
import { extensionQueryKeys } from './query-keys';

type Props = {
  outlet: 'navigation' | 'entity_preview_panel';
  context?: Record<string, unknown>;
};

/** A fixed host-owned insertion point; extensions never choose a DOM selector. */
export const ExtensionOutlet = ({ outlet, context }: Props) => {
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime,
    queryFn: getExtensionRuntime,
    retry: false,
  });
  if (runtime.isError) return null;
  return (
    <Stack spacing={1}>
      {runtime.data
        ?.filter((item) => item.kind === 'element' && item.outlet === outlet)
        .map((item) => (
          <ExtensionFrame
            contribution={item}
            context={context}
            key={`${item.extension_id}:${item.id}:${item.release_id}`}
          />
        ))}
    </Stack>
  );
};

export const ExtensionRoutePage = ({
  extensionId,
  contributionId,
}: {
  extensionId: string;
  contributionId: string;
}) => {
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime,
    queryFn: getExtensionRuntime,
    retry: false,
  });
  if (runtime.isPending) return null;
  const contribution = runtime.data?.find(
    (item) =>
      item.kind === 'route' &&
      item.extension_id === extensionId &&
      item.id === contributionId,
  );
  if (!contribution)
    return (
      <Alert severity="warning">This extension page is unavailable.</Alert>
    );
  return <ExtensionFrame contribution={contribution} />;
};
