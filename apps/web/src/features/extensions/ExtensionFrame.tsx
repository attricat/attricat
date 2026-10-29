import { Alert, Box, Collapse } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { useActionSelection } from './actionSelection';
import type { ExtensionContribution } from './api';
import {
  defaultExtensionFrameHeight,
  extensionFramePlaceholders,
  type ExtensionFramePlaceholder,
} from './constants';
import { ExtensionFrameLoading } from './ExtensionFrameLoading';
import { frameDocument } from './frameDocument';
import { contributionContext } from './outletContributions';
import { useExtensionFrame } from './useExtensionFrame';

const emptyContext: Record<string, unknown> = {};

type Props = {
  contribution: ExtensionContribution;
  context?: Record<string, unknown>;
  onContentHeight?: (height: number) => void;
  onFailure?: () => void;
  onReady?: () => void;
  /** What shows until the artifact renders; a skeleton by default. */
  placeholder?: ExtensionFramePlaceholder;
};

/** Executes one contribution in an opaque-origin document, never in Attricat's DOM. */
export const ExtensionFrame = ({
  contribution,
  context = emptyContext,
  onContentHeight,
  onFailure,
  onReady,
  placeholder = extensionFramePlaceholders.skeleton,
}: Props) => {
  const { t } = useTranslation();
  // A selection-aware outlet supplies its selection through context so the
  // shared action-bar and overflow layouts need no per-version knowledge.
  const selection = useActionSelection();
  const frameContext =
    contributionContext(contribution, context, selection) ?? emptyContext;
  const {
    errorKey,
    frameKey,
    handleFrameLoad,
    height,
    iframeRef,
    measured,
    ready,
  } = useExtensionFrame({
    contribution,
    context: frameContext,
    onContentHeight,
    onFailure,
    onReady,
  });
  const label = contribution.title ?? contribution.id;

  if (errorKey) return <Alert severity="warning">{t(errorKey)}</Alert>;
  const frame = (
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
  );
  // The collapsed frame stays mounted: its content is measured inside its own
  // document, so it can size itself before taking any space.
  if (placeholder === extensionFramePlaceholders.none)
    return <Collapse in={ready && measured && height > 0}>{frame}</Collapse>;
  return (
    <Box
      sx={{
        minHeight: ready ? 0 : defaultExtensionFrameHeight,
        position: 'relative',
      }}
    >
      {frame}
      {!ready && <ExtensionFrameLoading />}
    </Box>
  );
};
