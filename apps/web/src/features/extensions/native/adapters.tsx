import {
  Autocomplete,
  type AutocompleteRenderInputParams,
  Box,
  Button,
  Checkbox,
  Chip,
  Divider,
  FormControl,
  FormControlLabel,
  FormHelperText,
  IconButton,
  InputAdornment,
  Link,
  MenuItem,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TablePagination,
  TableRow,
  TableSortLabel,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { Link as RouterLink } from '@tanstack/react-router';
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '../../../components/EmptyState';
import { compactIconSize, smallIconSize } from '../../../components/iconSizes';
import type { HostIcon } from '../constants';
import { hostIconComponent, renderHostIcon } from '../hostIcons';
import { nativeUiBudgets } from './constants';

/** What the host gives every element adapter. */
export type AdapterProps = {
  /** Validated props, with each declared default filled in. */
  props: Record<string, unknown>;
  /** Drawn children: elements, or the element's plain text. */
  children: ReactNode;
  slots: Record<string, ReactNode>;
  /** Sends a declared event with the payload the adapter builds. */
  emit: (event: string, payload: unknown) => void;
  /** Whether the extension listens for an event. */
  listens: (event: string) => boolean;
  /** Hands the element's declared methods to the receiver. */
  provideMethods: (methods: Record<string, () => void>) => void;
};

/** Theme spacing for each UI kit spacing step. */
const spacing = { none: 0, xs: 0.5, sm: 1, md: 2, lg: 3, xl: 4 } as const;
const toneColors = {
  default: 'text.primary',
  muted: 'text.secondary',
  info: 'info.main',
  success: 'success.main',
  warning: 'warning.main',
  error: 'error.main',
} as const;
const badgeColors = {
  neutral: 'default',
  info: 'info',
  success: 'success',
  warning: 'warning',
  error: 'error',
} as const;
const alignments = {
  start: 'flex-start',
  center: 'center',
  end: 'flex-end',
  stretch: 'stretch',
  between: 'space-between',
} as const;
/** Heading levels within an outlet; the host's title sits above them. */
const headingVariants = { 1: 'h6', 2: 'subtitle1', 3: 'subtitle2' } as const;
const headingElements = { 1: 'h3', 2: 'h4', 3: 'h5' } as const;
const tablePageSize = 50;

const text = (value: unknown) =>
  typeof value === 'string' ? value : undefined;
const flag = (value: unknown) => value === true;

/** Calls `focus` on a host input when the extension asks for it. */
const useFocusMethod = <T extends HTMLElement = HTMLElement>(
  provideMethods: AdapterProps['provideMethods'],
) => {
  const ref = useRef<T>(null);
  useEffect(() => {
    provideMethods({ focus: () => ref.current?.focus() });
  }, [provideMethods]);
  return ref;
};

/**
 * Keeps typing local and sends changes once it pauses, flushing on blur and
 * submit. A new value from the extension replaces the draft only when it
 * differs from what the host last sent, so typing is never overwritten by an
 * echo of itself.
 */
const useDebouncedInput = <T,>(
  value: T,
  emit: AdapterProps['emit'],
  format: (value: T) => string,
  parse: (text: string) => T | undefined,
) => {
  const [draft, setDraft] = useState(() => format(value));
  const [seen, setSeen] = useState(value);
  const [sent, setSent] = useState(value);
  const timer = useRef<number | undefined>(undefined);
  if (!Object.is(value, seen)) {
    setSeen(value);
    if (!Object.is(value, sent)) {
      setSent(value);
      setDraft(format(value));
    }
  }
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const flush = (text: string) => {
    window.clearTimeout(timer.current);
    const next = parse(text);
    if (next === undefined || Object.is(next, sent)) return;
    setSent(next);
    emit('change', { value: next });
  };
  const change = (text: string) => {
    setDraft(text);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(
      () => flush(text),
      nativeUiBudgets.inputDebounce,
    );
  };
  return { draft, change, flush };
};

const sameText = (value: string) => value;

/** Label, help and error props every host input shares. */
const inputChrome = (props: Record<string, unknown>) => {
  const label = text(props.label) ?? '';
  const hidden = flag(props.hideLabel);
  const error = text(props.error);
  return {
    label: hidden ? undefined : label,
    slotProps: { htmlInput: { 'aria-label': hidden ? label : undefined } },
    error: Boolean(error),
    helperText: error ?? text(props.helperText),
    disabled: flag(props.disabled),
    required: flag(props.required),
  };
};

export const StackAdapter = ({ props, children }: AdapterProps) => (
  <Stack
    direction={props.direction === 'horizontal' ? 'row' : 'column'}
    spacing={spacing[props.gap as keyof typeof spacing]}
    sx={{
      alignItems: alignments[props.align as keyof typeof alignments],
      flexWrap: flag(props.wrap) ? 'wrap' : 'nowrap',
      justifyContent: alignments[props.justify as keyof typeof alignments],
    }}
    useFlexGap
  >
    {children}
  </Stack>
);

export const GridAdapter = ({ props, children }: AdapterProps) => (
  <Box
    sx={{
      display: 'grid',
      gap: spacing[props.gap as keyof typeof spacing],
      gridTemplateColumns: {
        xs: '1fr',
        sm: `repeat(${Number(props.columns)}, minmax(0, 1fr))`,
      },
    }}
  >
    {children}
  </Box>
);

export const DividerAdapter = () => <Divider />;

export const TextAdapter = ({ props, children }: AdapterProps) => (
  <Typography
    sx={{
      color: toneColors[props.tone as keyof typeof toneColors],
      fontFamily: flag(props.monospace) ? 'monospace' : undefined,
      fontWeight: props.emphasis === 'strong' ? 600 : undefined,
      whiteSpace: 'pre-wrap',
      overflowWrap: 'anywhere',
    }}
    variant={props.size === 'sm' ? 'body2' : 'body1'}
  >
    {children}
  </Typography>
);

export const HeadingAdapter = ({ props, children }: AdapterProps) => {
  const level = props.level as keyof typeof headingVariants;
  return (
    <Typography
      component={headingElements[level]}
      variant={headingVariants[level]}
    >
      {children}
    </Typography>
  );
};

export const ButtonAdapter = ({
  props,
  children,
  emit,
  provideMethods,
}: AdapterProps) => {
  const ref = useFocusMethod(provideMethods);
  const icon = text(props.icon) as HostIcon | undefined;
  return (
    <Button
      color={props.tone === 'danger' ? 'error' : 'primary'}
      disabled={flag(props.disabled)}
      loading={flag(props.loading)}
      onClick={() => emit('press', {})}
      ref={ref as React.Ref<HTMLButtonElement>}
      startIcon={icon ? renderHostIcon(icon, compactIconSize) : undefined}
      variant={
        props.variant === 'primary'
          ? 'contained'
          : props.variant === 'text'
            ? 'text'
            : 'outlined'
      }
    >
      {children}
    </Button>
  );
};

export const IconButtonAdapter = ({
  props,
  emit,
  provideMethods,
}: AdapterProps) => {
  const ref = useFocusMethod(provideMethods);
  const label = text(props.label) ?? '';
  return (
    <Tooltip title={label}>
      <span>
        <IconButton
          aria-label={label}
          color={props.tone === 'danger' ? 'error' : 'default'}
          disabled={flag(props.disabled)}
          onClick={() => emit('press', {})}
          ref={ref as React.Ref<HTMLButtonElement>}
        >
          {renderHostIcon(props.icon as HostIcon, smallIconSize)}
        </IconButton>
      </span>
    </Tooltip>
  );
};

export const TextFieldAdapter = ({
  props,
  emit,
  listens,
  provideMethods,
}: AdapterProps) => {
  const ref = useFocusMethod(provideMethods);
  const { draft, change, flush } = useDebouncedInput(
    text(props.value) ?? '',
    emit,
    sameText,
    sameText,
  );
  const suggestions = (props.suggestions as string[] | undefined) ?? [];
  const multiline = flag(props.multiline);
  const chrome = inputChrome(props);
  // Autocomplete hands its input wiring over in `slotProps`; keep it.
  const field = (
    params: Partial<AutocompleteRenderInputParams> & {
      value?: string;
      onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
    } = {},
  ) => (
    <TextField
      {...params}
      {...chrome}
      fullWidth
      inputRef={ref}
      multiline={multiline}
      onBlur={() => flush(draft)}
      onKeyDown={(event) => {
        if (event.key !== 'Enter' || multiline) return;
        // The last change goes first, so a host committing on Enter sees it.
        flush(draft);
        if (listens('submit')) emit('submit', { value: draft });
      }}
      placeholder={text(props.placeholder)}
      rows={multiline ? Number(props.rows ?? 3) : undefined}
      size="small"
      slotProps={{
        ...params.slotProps,
        htmlInput: {
          ...params.slotProps?.htmlInput,
          ...chrome.slotProps.htmlInput,
          maxLength: props.maxLength as number | undefined,
          readOnly: flag(props.readOnly),
          style: flag(props.monospace)
            ? { fontFamily: 'monospace' }
            : undefined,
        },
      }}
    />
  );
  if (!suggestions.length || multiline)
    return field({
      value: draft,
      onChange: (event: React.ChangeEvent<HTMLInputElement>) =>
        change(event.target.value),
    });
  // The extension computes complete suggestions; the host only lists them.
  return (
    <Autocomplete
      disableClearable
      disabled={chrome.disabled}
      filterOptions={(options) => options}
      freeSolo
      inputValue={draft}
      onChange={(_, value) => {
        if (typeof value !== 'string') return;
        change(value);
        flush(value);
      }}
      onInputChange={(_, value, reason) => {
        if (reason === 'input') change(value);
      }}
      options={suggestions}
      readOnly={flag(props.readOnly)}
      renderInput={(params) => field(params)}
      value={draft}
    />
  );
};

const formatNumber = (value: number | null) =>
  value === null ? '' : String(value);

const parseNumber = (value: string) => {
  if (value.trim() === '') return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
};

export const NumberFieldAdapter = ({
  props,
  emit,
  provideMethods,
}: AdapterProps) => {
  const ref = useFocusMethod(provideMethods);
  const { draft, change, flush } = useDebouncedInput<number | null>(
    typeof props.value === 'number' ? props.value : null,
    emit,
    formatNumber,
    parseNumber,
  );
  const chrome = inputChrome(props);
  const unit = text(props.unit);
  return (
    <TextField
      {...chrome}
      fullWidth
      inputRef={ref}
      onBlur={() => flush(draft)}
      onChange={(event) => change(event.target.value)}
      placeholder={text(props.placeholder)}
      size="small"
      slotProps={{
        htmlInput: {
          ...chrome.slotProps.htmlInput,
          inputMode: 'decimal',
          max: props.maximum as number | undefined,
          min: props.minimum as number | undefined,
          readOnly: flag(props.readOnly),
          step: props.step as number | undefined,
        },
        input: unit
          ? {
              endAdornment: (
                <InputAdornment position="end">{unit}</InputAdornment>
              ),
            }
          : undefined,
      }}
      type="number"
      value={draft}
    />
  );
};

type Option = { value: string; label: string; disabled?: boolean };

export const SelectAdapter = ({
  props,
  emit,
  provideMethods,
}: AdapterProps) => {
  const ref = useFocusMethod(provideMethods);
  const options = (props.options as Option[] | undefined) ?? [];
  const value = text(props.value) ?? '';
  const chrome = inputChrome(props);
  return (
    <TextField
      {...chrome}
      fullWidth
      inputRef={ref}
      onChange={(event) =>
        emit('change', { value: event.target.value || null })
      }
      select
      size="small"
      slotProps={{
        ...chrome.slotProps,
        select: {
          displayEmpty: Boolean(props.placeholder),
          inputProps: chrome.slotProps.htmlInput,
        },
      }}
      value={value}
    >
      {props.placeholder ? (
        <MenuItem disabled value="">
          {text(props.placeholder)}
        </MenuItem>
      ) : null}
      {options.map((option) => (
        <MenuItem
          disabled={option.disabled}
          key={option.value}
          value={option.value}
        >
          {option.label}
        </MenuItem>
      ))}
    </TextField>
  );
};

const toggle = (Control: typeof Checkbox | typeof Switch) =>
  function ToggleAdapter({ props, emit, provideMethods }: AdapterProps) {
    const ref = useFocusMethod<HTMLInputElement>(provideMethods);
    const helper = text(props.helperText);
    return (
      <FormControl disabled={flag(props.disabled)}>
        <FormControlLabel
          control={
            <Control
              checked={flag(props.checked)}
              slotProps={{ input: { ref } }}
              onChange={(event) =>
                emit('change', { checked: event.target.checked })
              }
            />
          }
          label={text(props.label) ?? ''}
        />
        {helper && <FormHelperText>{helper}</FormHelperText>}
      </FormControl>
    );
  };
export const CheckboxAdapter = toggle(Checkbox);
export const SwitchAdapter = toggle(Switch);

export const BadgeAdapter = ({ props, children }: AdapterProps) => {
  const icon = text(props.icon) as HostIcon | undefined;
  return (
    <Chip
      color={badgeColors[props.tone as keyof typeof badgeColors]}
      icon={icon ? renderHostIcon(icon, compactIconSize) : undefined}
      label={children}
      size="small"
      variant="outlined"
    />
  );
};

export const LinkAdapter = ({
  props,
  children,
  provideMethods,
}: AdapterProps) => {
  const ref = useFocusMethod(provideMethods);
  const href = text(props.href) ?? '';
  // The receiver accepted only `https:` URLs and in-app paths.
  if (href.startsWith('/'))
    return (
      <Link
        component={RouterLink}
        ref={ref as React.Ref<HTMLAnchorElement>}
        to={href}
      >
        {children}
      </Link>
    );
  return (
    <Link
      href={href}
      ref={ref as React.Ref<HTMLAnchorElement>}
      rel="noopener noreferrer"
      target="_blank"
    >
      {children}
    </Link>
  );
};

type Column = {
  key: string;
  label: string;
  align?: 'start' | 'end';
  sortable?: boolean;
};
type Row = { id: string; cells: Record<string, string | number | null> };

const compareCells = (left: unknown, right: unknown) => {
  if (left === right) return 0;
  if (left === null || left === undefined) return 1;
  if (right === null || right === undefined) return -1;
  if (typeof left === 'number' && typeof right === 'number')
    return left - right;
  return String(left).localeCompare(String(right));
};

/** Rows arrive as data, so the host sorts and pages them itself. */
export const TableAdapter = ({ props, emit, listens }: AdapterProps) => {
  const { t } = useTranslation();
  const columns = (props.columns as Column[] | undefined) ?? [];
  const rows = useMemo(
    () => (props.rows as Row[] | undefined) ?? [],
    [props.rows],
  );
  const [sort, setSort] = useState<{ key: string; descending: boolean }>();
  const [page, setPage] = useState(0);
  const sorted = useMemo(() => {
    if (!sort) return rows;
    const ordered = [...rows].sort((left, right) =>
      compareCells(left.cells[sort.key], right.cells[sort.key]),
    );
    return sort.descending ? ordered.reverse() : ordered;
  }, [rows, sort]);
  const visible = sorted.slice(
    page * tablePageSize,
    (page + 1) * tablePageSize,
  );
  const interactive = listens('rowPress');
  return (
    <Box sx={{ overflowX: 'auto' }}>
      <Table aria-label={text(props.label)} size="small">
        <TableHead>
          <TableRow>
            {columns.map((column) => (
              <TableCell
                align={column.align === 'end' ? 'right' : 'left'}
                key={column.key}
              >
                {column.sortable ? (
                  <TableSortLabel
                    active={sort?.key === column.key}
                    direction={
                      sort?.key === column.key && sort.descending
                        ? 'desc'
                        : 'asc'
                    }
                    onClick={() =>
                      setSort((current) => ({
                        key: column.key,
                        descending:
                          current?.key === column.key && !current.descending,
                      }))
                    }
                  >
                    {column.label}
                  </TableSortLabel>
                ) : (
                  column.label
                )}
              </TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          {visible.length === 0 ? (
            <TableRow>
              <TableCell colSpan={Math.max(columns.length, 1)}>
                <Typography color="text.secondary" variant="body2">
                  {text(props.emptyText) ?? t('extensions.native.tableEmpty')}
                </Typography>
              </TableCell>
            </TableRow>
          ) : (
            visible.map((row) => (
              <TableRow
                hover={interactive}
                key={row.id}
                onClick={
                  interactive
                    ? () => emit('rowPress', { id: row.id })
                    : undefined
                }
                onKeyDown={
                  interactive
                    ? (event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault();
                          emit('rowPress', { id: row.id });
                        }
                      }
                    : undefined
                }
                sx={interactive ? { cursor: 'pointer' } : undefined}
                tabIndex={interactive ? 0 : undefined}
              >
                {columns.map((column) => (
                  <TableCell
                    align={column.align === 'end' ? 'right' : 'left'}
                    key={column.key}
                  >
                    {row.cells[column.key] ?? ''}
                  </TableCell>
                ))}
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
      {sorted.length > tablePageSize && (
        <TablePagination
          component="div"
          count={sorted.length}
          onPageChange={(_, next) => setPage(next)}
          page={page}
          rowsPerPage={tablePageSize}
          rowsPerPageOptions={[tablePageSize]}
        />
      )}
    </Box>
  );
};

export const EmptyStateAdapter = ({ props, slots }: AdapterProps) => (
  <Box>
    <EmptyState
      description={text(props.description)}
      icon={hostIconComponent(props.icon as HostIcon | undefined)}
      title={text(props.title) ?? ''}
    />
    {slots.action && (
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: -4, pb: 4 }}>
        {slots.action}
      </Box>
    )}
  </Box>
);
