import { Alert, Box } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { ExtensionContribution } from './api';
import { defaultExtensionFrameHeight } from './constants';
import { ExtensionFrameLoading } from './ExtensionFrameLoading';
import { frameDocument } from './frameDocument';
import { useExtensionFrame } from './useExtensionFrame';

const emptyContext: Record<string, unknown> = {};

type Props = {
  contribution: ExtensionContribution;
  context?: Record<string, unknown>;
  onContentHeight?: (height: number) => void;
  onFailure?: () => void;
  onReady?: () => void;
};

/** Executes one contribution in an opaque-origin document, never in Catalog's DOM. */
export const ExtensionFrame = ({
  contribution,
  context = emptyContext,
  onContentHeight,
  onFailure,
  onReady,
}: Props) => {
  const { t } = useTranslation();
  const { errorKey, frameKey, handleFrameLoad, height, iframeRef, ready } =
    useExtensionFrame({
      contribution,
      context,
      onContentHeight,
      onFailure,
      onReady,
    });
  const label = contribution.title ?? contribution.id;

  if (errorKey) return <Alert severity="warning">{t(errorKey)}</Alert>;
  return (
    <Box
      sx={{
        minHeight: ready ? 0 : defaultExtensionFrameHeight,
        position: 'relative',
      }}
    >
      <iframe
        aria-label={label}
        key={frameKey}
        onLoad={handleFrameLoad}
        ref={iframeRef}
        sandbox="allow-scripts"
        srcDoc={frameDocument}
        style={{ border: 0, height, opacity: ready ? 1 : 0, width: '100%' }}
        title={label}
      />
      {!ready && <ExtensionFrameLoading />}
    </Box>
  );
};
