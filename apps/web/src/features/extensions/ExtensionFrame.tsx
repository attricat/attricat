import { Alert, Box, Collapse } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { ExtensionContribution } from './api';
import {
  defaultExtensionFrameHeight,
  extensionFramePlaceholders,
  type ExtensionFramePlaceholder,
} from './constants';
import { ExtensionFrameLoading } from './ExtensionFrameLoading';
import { frameDocument } from './frameDocument';
import { useExtensionFrame } from './useExtensionFrame';
import { useContributionTitle } from './contributionTitle';

const emptyContext: Record<string, unknown> = {};

type Props = {
  contribution: ExtensionContribution;
  context?: Record<string, unknown>;
  onContentHeight?: (height: number) => void;
  onFailure?: () => void;
  onReady?: () => void;
  /** What shows until the artifact renders; a skeleton by default. */
  placeholder?: ExtensionFramePlaceholder;
  /** Names what the contribution belongs to; it collapses with the frame. */
  heading?: ReactNode;
};

/** Executes one contribution in an opaque-origin document, never in Attricat's DOM. */
export const ExtensionFrame = ({
  contribution,
  context = emptyContext,
  onContentHeight,
  onFailure,
  onReady,
  placeholder = extensionFramePlaceholders.skeleton,
  heading,
}: Props) => {
  const { t } = useTranslation();
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
    context,
    onContentHeight,
    onFailure,
    onReady,
  });
  const label = useContributionTitle()(contribution);

  if (errorKey) return <Alert severity="warning">{t(errorKey)}</Alert>;
  // A block frame: an inline one keeps a text line's height even when empty.
  const frame = (
    <iframe
      aria-label={label}
      key={frameKey}
      onLoad={handleFrameLoad}
      ref={iframeRef}
      sandbox="allow-scripts"
      srcDoc={frameDocument}
      style={{
        border: 0,
        display: 'block',
        height,
        opacity: ready ? 1 : 0,
        width: '100%',
      }}
      title={label}
    />
  );
  // The collapsed frame stays mounted: its content is measured inside its own
  // document, so it can size itself before taking any space.
  if (placeholder === extensionFramePlaceholders.none)
    return (
      <Collapse in={ready && measured && height > 0}>
        {heading}
        {frame}
      </Collapse>
    );
  return (
    <Box
      sx={{
        minHeight: ready ? 0 : defaultExtensionFrameHeight,
        position: 'relative',
      }}
    >
      {heading}
      {frame}
      {!ready && <ExtensionFrameLoading />}
    </Box>
  );
};
