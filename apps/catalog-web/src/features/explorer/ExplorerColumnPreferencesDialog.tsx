import { RestrictToVerticalAxis } from '@dnd-kit/abstract/modifiers';
import { Accessibility, type Draggable } from '@dnd-kit/dom';
import { move } from '@dnd-kit/helpers';
import { DragDropProvider } from '@dnd-kit/react';
import { isSortable, useSortable } from '@dnd-kit/react/sortable';
import {
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  List,
  ListItem,
  ListItemText,
  Tooltip,
} from '@mui/material';
import { ArrowDownIcon, ArrowUpIcon, GripVerticalIcon } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { ValueTypeIcon } from '../records/components/ValueTypeLabel';
import type { AttributeValueType } from '../records/valueTypeIcons';
import type { ExplorerColumnPreferences } from './columnPreferences';
import { columnPreferencesListMaxHeight } from './constants';

type Props = {
  columns: { id: string; label: string; valueType?: AttributeValueType }[];
  onChange: (preferences: ExplorerColumnPreferences) => void;
  onClear: () => void;
  onClose: () => void;
  open: boolean;
  preferences: ExplorerColumnPreferences;
};

// `Accessibility.configure` is untyped, so take its options from the plugin.
type AccessibilityOptions = NonNullable<
  ConstructorParameters<typeof Accessibility>[1]
>;

type SortableColumnProps = {
  count: number;
  hidden: boolean;
  id: string;
  index: number;
  label: string;
  onMove: (direction: -1 | 1) => void;
  onToggle: () => void;
  valueType?: AttributeValueType;
};

const SortableColumn = ({
  count,
  hidden,
  id,
  index,
  label,
  onMove,
  onToggle,
  valueType,
}: SortableColumnProps) => {
  const { t } = useTranslation();
  const { handleRef, isDragSource, ref } = useSortable({ id, index });

  return (
    <ListItem
      ref={ref}
      secondaryAction={
        <>
          <Tooltip title={t('explorer.moveColumnUp')}>
            <span>
              <IconButton
                aria-label={t('explorer.moveColumnUp')}
                disabled={index === 0}
                onClick={() => onMove(-1)}
                size="small"
              >
                <ArrowUpIcon />
              </IconButton>
            </span>
          </Tooltip>
          <Tooltip title={t('explorer.moveColumnDown')}>
            <span>
              <IconButton
                aria-label={t('explorer.moveColumnDown')}
                disabled={index === count - 1}
                onClick={() => onMove(1)}
                size="small"
              >
                <ArrowDownIcon />
              </IconButton>
            </span>
          </Tooltip>
        </>
      }
      sx={{
        bgcolor: 'background.paper',
        borderRadius: 1,
        boxShadow: isDragSource ? 3 : 0,
        // Shadows barely show on dark paper, so outline the lifted row too.
        outline: (theme) =>
          isDragSource ? `1px solid ${theme.palette.divider}` : 'none',
        zIndex: isDragSource ? 1 : undefined,
      }}
    >
      <IconButton
        aria-label={t('explorer.dragColumn', { column: label })}
        edge="start"
        ref={handleRef}
        size="small"
        sx={{
          cursor: isDragSource ? 'grabbing' : 'grab',
          mr: 0.5,
          touchAction: 'none',
        }}
      >
        <GripVerticalIcon />
      </IconButton>
      <Checkbox
        checked={!hidden}
        slotProps={{
          input: {
            'aria-label': t('explorer.showColumn', { column: label }),
          },
        }}
        onChange={onToggle}
      />
      {/* Built-in columns have no value type; keep their labels aligned. */}
      <Box
        sx={{
          display: 'inline-flex',
          flexShrink: 0,
          mr: 3,
          width: compactIconSize,
        }}
      >
        {valueType && <ValueTypeIcon valueType={valueType} />}
      </Box>
      <ListItemText primary={label} />
    </ListItem>
  );
};

export const ExplorerColumnPreferencesDialog = ({
  columns,
  onChange,
  onClear,
  onClose,
  open,
  preferences,
}: Props) => {
  const { t } = useTranslation();
  const labels = useMemo(
    () => new Map(columns.map((column) => [column.id, column.label])),
    [columns],
  );
  const valueTypes = useMemo(
    () => new Map(columns.map((column) => [column.id, column.valueType])),
    [columns],
  );
  const count = preferences.order.length;
  const moveBy = (id: string, direction: -1 | 1) => {
    const index = preferences.order.indexOf(id);
    const target = index + direction;
    if (target < 0 || target >= count) return;
    const order = [...preferences.order];
    [order[index], order[target]] = [order[target], order[index]];
    onChange({ ...preferences, order });
  };
  // dnd-kit announces IDs in English by default; announce column labels
  // and positions in the user's language instead.
  const plugins = useMemo(() => {
    const label = (id: string | number) => labels.get(String(id)) ?? String(id);
    const announce = (key: string, source: Draggable | null) =>
      source
        ? t(key, {
            column: label(source.id),
            position: isSortable(source) ? source.index + 1 : undefined,
            count,
          })
        : undefined;
    const options: AccessibilityOptions = {
      announcements: {
        dragstart: ({ operation: { source } }) =>
          announce('explorer.columnDragStart', source),
        dragover: ({ operation: { source } }) =>
          announce('explorer.columnDragOver', source),
        dragend: ({ operation: { source }, canceled }) =>
          announce(
            canceled ? 'explorer.columnDragCancel' : 'explorer.columnDragEnd',
            source,
          ),
      },
      screenReaderInstructions: {
        draggable: t('explorer.columnDragInstructions'),
      },
    };
    const accessibility = Accessibility.configure(options);
    return <T,>(defaults: T[]) =>
      defaults.map((plugin) =>
        plugin === Accessibility ? accessibility : plugin,
      );
  }, [count, labels, t]);

  return (
    <Dialog fullWidth maxWidth="sm" onClose={onClose} open={open}>
      <DialogTitle>{t('explorer.columnPreferences')}</DialogTitle>
      <DialogContent>
        <Box
          sx={{ maxHeight: columnPreferencesListMaxHeight, overflowY: 'auto' }}
        >
          <DragDropProvider
            modifiers={[RestrictToVerticalAxis]}
            onDragEnd={(event) => {
              if (event.canceled) return;
              const order = move(preferences.order, event);
              if (order !== preferences.order)
                onChange({ ...preferences, order });
            }}
            plugins={plugins}
          >
            <List aria-label={t('explorer.columns')}>
              {preferences.order.map((id, index) => (
                <SortableColumn
                  count={count}
                  hidden={preferences.hidden.includes(id)}
                  id={id}
                  index={index}
                  key={id}
                  label={labels.get(id) ?? id}
                  onMove={(direction) => moveBy(id, direction)}
                  onToggle={() =>
                    onChange({
                      ...preferences,
                      hidden: preferences.hidden.includes(id)
                        ? preferences.hidden.filter((hidden) => hidden !== id)
                        : [...preferences.hidden, id],
                    })
                  }
                  valueType={valueTypes.get(id)}
                />
              ))}
            </List>
          </DragDropProvider>
        </Box>
      </DialogContent>
      <DialogActions>
        <Button color="inherit" onClick={onClear}>
          {t('explorer.clearColumnPreferences')}
        </Button>
        <Button onClick={onClose} variant="contained">
          {t('explorer.done')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
