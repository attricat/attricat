import { Box } from '@mui/material';
import { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { editorShareLimits, editorShareStep } from './definitionLayout';

const percent = (share: number) => Math.round(share * 100);

/**
 * The divider between the definition editor and its preview. Dragging it,
 * or pressing the arrow keys while it has focus, resizes them.
 */
export const DefinitionSplitHandle = ({
  editorShare,
  onCommit,
  onResize,
}: {
  editorShare: number;
  /** Keeps a width once the drag or key press ends. */
  onCommit: (share: number) => void;
  /** Shows a width while dragging. */
  onResize: (share: number) => void;
}) => {
  const { t } = useTranslation();
  const latest = useRef(editorShare);
  const shareAt = (handle: HTMLElement, clientX: number) => {
    const workspace = handle.parentElement?.getBoundingClientRect();
    return workspace && workspace.width > 0
      ? (clientX - workspace.left) / workspace.width
      : editorShare;
  };
  const keyShares: Record<string, number> = {
    ArrowLeft: editorShare - editorShareStep,
    ArrowRight: editorShare + editorShareStep,
    End: editorShareLimits.max,
    Home: editorShareLimits.min,
  };
  return (
    <Box
      aria-label={t('definitionEditor.layout.resize')}
      aria-orientation="vertical"
      aria-valuemax={percent(editorShareLimits.max)}
      aria-valuemin={percent(editorShareLimits.min)}
      aria-valuenow={percent(editorShare)}
      onKeyDown={(event) => {
        const share = keyShares[event.key];
        if (share === undefined) return;
        event.preventDefault();
        onCommit(share);
      }}
      onLostPointerCapture={() => onCommit(latest.current)}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        latest.current = editorShare;
      }}
      onPointerMove={(event) => {
        if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
        latest.current = shareAt(event.currentTarget, event.clientX);
        onResize(latest.current);
      }}
      role="separator"
      sx={{
        cursor: 'col-resize',
        display: { lg: 'flex', xs: 'none' },
        justifyContent: 'center',
        outline: 'none',
        touchAction: 'none',
        width: (theme) => theme.spacing(2),
        '&::after': {
          bgcolor: 'divider',
          borderRadius: 1,
          content: '""',
          transition: (theme) => theme.transitions.create('background-color'),
          width: 2,
        },
        '&:hover::after, &:focus-visible::after': { bgcolor: 'primary.main' },
      }}
      tabIndex={0}
    />
  );
};
