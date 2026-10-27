import { Accessibility, type Draggable } from '@dnd-kit/dom';
import { move as moveDragged } from '@dnd-kit/helpers';
import { DragDropProvider } from '@dnd-kit/react';
import { isSortable, useSortable } from '@dnd-kit/react/sortable';
import { Box, IconButton, Stack, Tooltip } from '@mui/material';
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  GripVerticalIcon,
  Trash2Icon,
} from 'lucide-react';
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { ImageGallery, type GalleryFile } from './ImageGallery';
import { ConfirmDialog } from '../../components/ConfirmDialog';

// `Accessibility.configure` is untyped, so take its options from the plugin.
type AccessibilityOptions = NonNullable<
  ConstructorParameters<typeof Accessibility>[1]
>;

// The handle sits among the tile's actions, away from the hook that owns it.
const DragHandleContext = createContext<{
  dragging: boolean;
  handleRef: (element: Element | null) => void;
} | null>(null);

const SortableTile = ({
  children,
  disabled,
  id,
  index,
}: {
  children: ReactNode;
  disabled: boolean;
  id: string;
  index: number;
}) => {
  const { handleRef, isDragSource, ref } = useSortable({ disabled, id, index });
  const handle = useMemo(
    () => ({ dragging: isDragSource, handleRef }),
    [handleRef, isDragSource],
  );
  return (
    <Box
      ref={ref}
      sx={{
        minWidth: 0,
        zIndex: isDragSource ? 1 : undefined,
        // The card's border keeps the lifted card distinct on dark paper.
        '& > .MuiCard-root': { boxShadow: isDragSource ? 3 : 'none' },
      }}
    >
      <DragHandleContext.Provider value={handle}>
        {children}
      </DragHandleContext.Provider>
    </Box>
  );
};

const DragHandle = ({
  disabled,
  label,
}: {
  disabled: boolean;
  label: string;
}) => {
  const handle = useContext(DragHandleContext);
  return (
    <IconButton
      aria-label={label}
      disabled={disabled}
      ref={handle?.handleRef}
      sx={{
        cursor: handle?.dragging ? 'grabbing' : 'grab',
        touchAction: 'none',
      }}
    >
      <GripVerticalIcon />
    </IconButton>
  );
};

// Focus moves to the opposite arrow when the clicked one ends up disabled.
const oppositeMove = { earlier: 'later', later: 'earlier' } as const;

const GalleryAction = ({
  label,
  color,
  disabled,
  move,
  onClick,
  children,
}: {
  label: string;
  color?: 'error';
  disabled: boolean;
  move?: keyof typeof oppositeMove;
  onClick: () => void;
  children: ReactNode;
}) => (
  <Tooltip title={label}>
    {/* Disabled buttons emit no events; the span keeps the tooltip working. */}
    <span>
      <IconButton
        aria-label={label}
        color={color}
        disabled={disabled}
        onClick={onClick}
        data-gallery-move={move}
      >
        {children}
      </IconButton>
    </span>
  </Tooltip>
);

export const ImageGalleryEditor = ({
  files,
  disabled,
  ordered,
  pendingTiles,
  onChange,
}: {
  files: readonly GalleryFile[];
  disabled: boolean;
  pendingTiles?: readonly ReactNode[];
  ordered: boolean;
  onChange: (fileIds: string[]) => Promise<boolean>;
}) => {
  const { t } = useTranslation();
  const [removeId, setRemoveId] = useState<string | null>(null);
  // Shown until the save settles, so a dropped tile does not jump back.
  const [pendingOrder, setPendingOrder] = useState<string[] | null>(null);
  const [removing, setRemoving] = useState(false);
  const removedFile = files.find((file) => file.id === removeId);
  const shown = useMemo(() => {
    if (!pendingOrder) return files;
    const byId = new Map(files.map((file) => [file.id, file]));
    const sorted = pendingOrder.flatMap((id) => byId.get(id) ?? []);
    return sorted.length === files.length ? sorted : files;
  }, [files, pendingOrder]);
  // Saving disables every control, which drops keyboard focus to the page.
  // Return it to the control that made the change once the save settles.
  const focusAfterSave = useRef<HTMLElement | null>(null);
  useEffect(() => {
    const element = focusAfterSave.current;
    if (disabled || pendingOrder || !element) return;
    focusAfterSave.current = null;
    // The tile moved to another gallery page.
    if (!element.isConnected) return;
    const move = element.dataset.galleryMove;
    const target =
      element.matches(':disabled') && (move === 'earlier' || move === 'later')
        ? element
            .closest('[data-gallery-actions]')
            ?.querySelector<HTMLElement>(
              `[data-gallery-move="${oppositeMove[move]}"]`,
            )
        : element;
    target?.focus();
  }, [disabled, pendingOrder]);
  const reorder = async (ids: string[]) => {
    const focused = document.activeElement;
    if (
      focused instanceof HTMLElement &&
      focused.closest('[data-gallery-actions]')
    )
      focusAfterSave.current = focused;
    setPendingOrder(ids);
    try {
      await onChange(ids);
    } finally {
      setPendingOrder(null);
    }
  };
  const move = (index: number, offset: number) => {
    if (disabled || !ordered) return;
    const ids = shown.map((file) => file.id);
    [ids[index], ids[index + offset]] = [ids[index + offset], ids[index]];
    void reorder(ids);
  };
  // dnd-kit announces IDs in English by default; announce file names and
  // positions in the user's language instead.
  const count = shown.length;
  const plugins = useMemo(() => {
    const names = new Map(shown.map((file) => [file.id, file.filename]));
    const announce = (key: string, source: Draggable | null) =>
      source
        ? t(key, {
            filename: names.get(String(source.id)) ?? String(source.id),
            position: isSortable(source) ? source.index + 1 : undefined,
            count,
          })
        : undefined;
    const options: AccessibilityOptions = {
      announcements: {
        dragstart: ({ operation: { source } }) =>
          announce('files.imageDragStart', source),
        dragover: ({ operation: { source } }) =>
          announce('files.imageDragOver', source),
        dragend: ({ operation: { source }, canceled }) =>
          announce(
            canceled ? 'files.imageDragCancel' : 'files.imageDragEnd',
            source,
          ),
      },
      screenReaderInstructions: {
        draggable: t('files.imageDragInstructions'),
      },
    };
    const accessibility = Accessibility.configure(options);
    return <T,>(defaults: T[]) =>
      defaults.map((plugin) =>
        plugin === Accessibility ? accessibility : plugin,
      );
  }, [count, shown, t]);
  const gallery = (
    <ImageGallery
      files={shown}
      pendingTiles={pendingTiles}
      renderItem={
        ordered
          ? ({ children, file, index }) => (
              <SortableTile
                disabled={disabled}
                id={file.id}
                index={index}
                key={file.id}
              >
                {children}
              </SortableTile>
            )
          : undefined
      }
      renderActions={(file, index) => (
        <Stack
          data-gallery-actions
          direction="row"
          sx={{ alignItems: 'center', justifyContent: 'space-between' }}
        >
          {ordered && (
            <Stack direction="row">
              <DragHandle
                disabled={disabled}
                label={t('files.dragImage', { filename: file.filename })}
              />
              <GalleryAction
                label={t('files.moveEarlier', { filename: file.filename })}
                disabled={disabled || index === 0}
                move="earlier"
                onClick={() => move(index, -1)}
              >
                <ArrowLeftIcon />
              </GalleryAction>
              <GalleryAction
                label={t('files.moveLater', { filename: file.filename })}
                disabled={disabled || index === count - 1}
                move="later"
                onClick={() => move(index, 1)}
              >
                <ArrowRightIcon />
              </GalleryAction>
            </Stack>
          )}
          {/* Removal sits apart at the end, in the destructive colour. */}
          <Box sx={{ ml: 'auto' }}>
            <GalleryAction
              label={t('files.removeImage', { filename: file.filename })}
              color="error"
              disabled={disabled}
              onClick={() => {
                if (!disabled) setRemoveId(file.id);
              }}
            >
              <Trash2Icon />
            </GalleryAction>
          </Box>
        </Stack>
      )}
    />
  );
  return (
    <>
      {ordered ? (
        <DragDropProvider
          onDragEnd={(event) => {
            if (event.canceled || disabled) return;
            const ids = shown.map((file) => file.id);
            const next = moveDragged(ids, event);
            if (next !== ids) void reorder(next);
          }}
          plugins={plugins}
        >
          {gallery}
        </DragDropProvider>
      ) : (
        gallery
      )}
      <ConfirmDialog
        confirmLabel={t('files.confirmRemove')}
        confirmDisabled={disabled}
        destructive
        error={null}
        isPending={removing}
        onClose={() => setRemoveId(null)}
        onConfirm={async () => {
          if (disabled) return;
          setRemoving(true);
          // Close on failure too, so inline server errors are not hidden by the dialog.
          try {
            await onChange(
              files
                .filter((file) => file.id !== removeId)
                .map((file) => file.id),
            );
          } finally {
            setRemoving(false);
            setRemoveId(null);
          }
        }}
        open={Boolean(removedFile)}
        title={t('files.removeImage', { filename: removedFile?.filename })}
      >
        {t('files.removeImageExplanation')}
      </ConfirmDialog>
    </>
  );
};
