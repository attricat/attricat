import { Box } from '@mui/material';
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { ExtensionFrame } from '../extensions/ExtensionFrame';
import type { ExtensionContribution } from '../extensions/api';
import { recordFrameTiming } from '../inspector/timing';

import type { ExplorerTableCellContext } from './schemas';

const cellStartTimeout = 1_500;

type Props = {
  contribution?: ExtensionContribution;
  context: ExplorerTableCellContext;
  fallback: ReactNode;
  frameAllowed: boolean;
};

/**
 * A cell never delays the Explorer: the normal scalar is shown until its
 * sandbox has started, and permanently if the renderer is unavailable, slow,
 * or fails. The parent only creates this component for virtualized cells.
 */
const ExtensionTableCellContent = ({
  contribution,
  context,
  fallback,
  frameAllowed,
}: Props) => {
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const startedAt = useRef<number | null>(null);

  useEffect(() => {
    startedAt.current = performance.now();
  }, []);

  useEffect(() => {
    if (!contribution || !frameAllowed || ready || failed) return;
    const timer = window.setTimeout(() => setFailed(true), cellStartTimeout);
    return () => window.clearTimeout(timer);
  }, [contribution, failed, frameAllowed, ready]);

  const handleFailure = useCallback(() => {
    setFailed((wasFailed) => {
      if (!wasFailed && startedAt.current !== null) {
        recordFrameTiming(
          'frame-fallback',
          performance.now() - startedAt.current,
        );
      }
      return true;
    });
  }, []);
  const handleReady = useCallback(() => {
    setReady((wasReady) => {
      if (!wasReady && startedAt.current !== null) {
        recordFrameTiming('frame-load', performance.now() - startedAt.current);
      }
      return true;
    });
  }, []);
  if (!contribution || !frameAllowed || failed) return <>{fallback}</>;
  return (
    <Box sx={{ minWidth: 0 }}>
      {!ready && fallback}
      <Box
        sx={
          ready
            ? undefined
            : { height: 0, overflow: 'hidden', visibility: 'hidden' }
        }
      >
        <ExtensionFrame
          context={context}
          contribution={contribution}
          onFailure={handleFailure}
          onReady={handleReady}
        />
      </Box>
    </Box>
  );
};

export const ExtensionTableCell = (props: Props) => (
  <ExtensionTableCellContent
    key={`${props.contribution?.release_id ?? ''}:${JSON.stringify(props.context)}`}
    {...props}
  />
);
