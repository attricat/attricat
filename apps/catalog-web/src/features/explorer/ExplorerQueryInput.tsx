import {
  Box,
  CircularProgress,
  FormHelperText,
  IconButton,
  InputAdornment,
  Paper,
  Popper,
  Stack,
  TextField,
  Tooltip,
  Typography,
  type InputBaseComponentProps,
} from '@mui/material';
import {
  AsteriskIcon,
  BracesIcon,
  CalendarIcon,
  CircleQuestionMarkIcon,
  ClockIcon,
  CornerDownRightIcon,
  FileIcon,
  HashIcon,
  ListIcon,
  ToggleLeftIcon,
  TypeIcon,
  type LucideIcon,
} from 'lucide-react';
import {
  createContext,
  useContext,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type Ref,
  type SyntheticEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { compactIconSize, smallIconSize } from '../../components/iconSizes';
import { EntityIcon, RelationshipIcon } from '../../components/systemIcons';
import type { Blueprint, BlueprintWithAttributes } from '../entities/api';
import { attributeLabel } from '../entities/entityDisplay';
import { attributeValueTypes } from '../entities/valueTypes';
import { querySuggestionListMaxHeight } from './constants';
import {
  firstQueryError,
  highlightQuery,
  type QueryError,
  type QuerySegment,
} from './queryLanguage';
import {
  applyQuerySuggestion,
  querySuggestions,
  type QuerySuggestion,
} from './querySuggestions';
import { useQuerySchema } from './useQuerySchema';

const valueTypeIcons: Record<string, LucideIcon> = {
  [attributeValueTypes.string]: TypeIcon,
  [attributeValueTypes.number]: HashIcon,
  [attributeValueTypes.integer]: HashIcon,
  [attributeValueTypes.boolean]: ToggleLeftIcon,
  [attributeValueTypes.date]: CalendarIcon,
  [attributeValueTypes.datetime]: CalendarIcon,
  [attributeValueTypes.time]: ClockIcon,
  [attributeValueTypes.json]: BracesIcon,
  [attributeValueTypes.file]: FileIcon,
};

const suggestionIcon = (suggestion: QuerySuggestion): LucideIcon => {
  switch (suggestion.kind) {
    case 'attribute':
      return valueTypeIcons[suggestion.attribute.value_type] ?? TypeIcon;
    case 'relationship':
      return RelationshipIcon;
    case 'allFields':
      return ListIcon;
    case 'global':
      return AsteriskIcon;
    case 'ids':
      return EntityIcon;
    case 'value':
      return CornerDownRightIcon;
  }
};

const QuerySegmentsContext = createContext<QuerySegment[]>([]);

const segmentColor: Record<QuerySegment['kind'], string> = {
  field: 'primary.main',
  global: 'secondary.main',
  punctuation: 'text.secondary',
  value: 'text.primary',
  wildcard: 'secondary.main',
  whitespace: 'text.primary',
};

/**
 * Renders the native input with transparent text and draws the highlighted
 * query on top of it. Both share one grid cell and the input's class names,
 * so their boxes, padding, and glyphs line up; the overlay ignores pointer
 * events so editing stays native.
 */
const HighlightedInput = ({
  className,
  ref,
  onScroll,
  ...props
}: InputBaseComponentProps & { ref?: Ref<HTMLInputElement> }) => {
  const segments = useContext(QuerySegmentsContext);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const overlayRef = useRef<HTMLDivElement>(null);
  const syncScroll = () => {
    if (inputRef.current && overlayRef.current) {
      overlayRef.current.scrollLeft = inputRef.current.scrollLeft;
    }
  };
  useLayoutEffect(syncScroll);
  return (
    <Box
      sx={{
        display: 'grid',
        flex: 1,
        gridTemplateColumns: 'minmax(0, 1fr)',
        minWidth: 0,
      }}
    >
      <input
        {...props}
        className={className}
        onScroll={(event) => {
          syncScroll();
          onScroll?.(event);
        }}
        onSelect={(event) => {
          syncScroll();
          props.onSelect?.(event);
        }}
        ref={(element) => {
          inputRef.current = element;
          if (typeof ref === 'function') ref(element);
          else if (ref) ref.current = element;
        }}
        style={{ ...props.style, color: 'transparent', gridArea: '1 / 1' }}
      />
      <Box
        aria-hidden
        className={className}
        ref={overlayRef}
        sx={{
          gridArea: '1 / 1',
          overflow: 'hidden',
          pointerEvents: 'none',
          whiteSpace: 'pre',
        }}
      >
        {segments.map((segment) => (
          <Box
            component="span"
            key={segment.start}
            sx={{
              color: segment.error ? 'error.main' : segmentColor[segment.kind],
              ...(segment.error && {
                textDecorationColor: (theme) => theme.palette.error.main,
                textDecorationLine: 'underline',
                textDecorationStyle: 'wavy',
                textDecorationSkipInk: 'none',
              }),
            }}
          >
            {segment.text}
          </Box>
        ))}
      </Box>
    </Box>
  );
};

type Props = {
  value: string;
  onChange: (value: string) => void;
  /** Selected blueprint schema; highlighting and completion need it. */
  blueprint?: BlueprintWithAttributes;
  blueprints: Blueprint[];
  onShowSyntax: (anchor: HTMLElement) => void;
  /**
   * Where to render query errors. Rendering them outside the field keeps
   * neighbouring controls in a row from stretching to the helper text.
   */
  errorContainer?: HTMLElement | null;
};

export const ExplorerQueryInput = ({
  value,
  onChange,
  blueprint,
  blueprints,
  onShowSyntax,
  errorContainer,
}: Props) => {
  const { t } = useTranslation();
  const listboxId = useId();
  const errorId = useId();
  const optionId = (index: number) => `${listboxId}-option-${index}`;
  const [anchor, setAnchor] = useState<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const pendingCursor = useRef<number | null>(null);
  const [cursor, setCursor] = useState(value.length);
  const [open, setOpen] = useState(false);
  const [focused, setFocused] = useState(false);
  const [active, setActive] = useState(-1);
  const { schema, blueprintNames, targetsLoading } = useQuerySchema(
    blueprint,
    blueprints,
    value,
  );
  const segments = useMemo(
    () => highlightQuery(value, schema),
    [value, schema],
  );
  const error = firstQueryError(segments, value, focused ? cursor : undefined);
  const completion = schema
    ? querySuggestions(
        schema,
        value,
        Math.min(cursor, value.length),
        blueprintNames,
      )
    : undefined;
  const suggestions = completion?.suggestions ?? [];
  const valueHint =
    completion?.context.kind === 'value'
      ? completion.context.selector
      : undefined;
  const loading = Boolean(completion?.loading && targetsLoading);
  const showPopup =
    open && focused && Boolean(suggestions.length || loading || valueHint);
  const activeIndex = active < suggestions.length ? active : -1;

  useLayoutEffect(() => {
    if (pendingCursor.current === null || !inputRef.current) return;
    inputRef.current.setSelectionRange(
      pendingCursor.current,
      pendingCursor.current,
    );
    pendingCursor.current = null;
  });
  useLayoutEffect(() => {
    if (activeIndex < 0) return;
    listRef.current
      ?.querySelector(`#${CSS.escape(optionId(activeIndex))}`)
      ?.scrollIntoView({ block: 'nearest' });
  });

  const trackCursor = (
    event: SyntheticEvent<HTMLInputElement | HTMLTextAreaElement>,
  ) =>
    setCursor(
      event.currentTarget.selectionStart ?? event.currentTarget.value.length,
    );

  const accept = (suggestion: QuerySuggestion) => {
    if (!completion) return;
    const next = applyQuerySuggestion(value, completion.context, suggestion);
    pendingCursor.current = next.cursor;
    onChange(next.query);
    setCursor(next.cursor);
    setActive(-1);
    setOpen(true);
    inputRef.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === ' ' && event.ctrlKey) {
      event.preventDefault();
      setOpen(true);
      return;
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      if (!suggestions.length) return;
      event.preventDefault();
      if (!showPopup) {
        setOpen(true);
        setActive(event.key === 'ArrowDown' ? 0 : suggestions.length - 1);
        return;
      }
      // Cycle through the suggestions and back to the typed text (-1).
      const next = activeIndex + (event.key === 'ArrowDown' ? 1 : -1);
      setActive(
        next >= suggestions.length
          ? -1
          : next < -1
            ? suggestions.length - 1
            : next,
      );
      return;
    }
    if (!showPopup) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      return;
    }
    // Enter submits the search unless a suggestion was chosen with the
    // arrow keys; Tab completes the highlighted or first suggestion.
    const chosen =
      event.key === 'Enter'
        ? suggestions[activeIndex]
        : event.key === 'Tab' && !event.shiftKey
          ? suggestions[Math.max(activeIndex, 0)]
          : undefined;
    if (chosen) {
      event.preventDefault();
      accept(chosen);
    }
  };

  const describe = (suggestion: QuerySuggestion) => {
    switch (suggestion.kind) {
      case 'attribute': {
        const valueSchema = suggestion.attribute.value_schema;
        const schemaDescription =
          typeof valueSchema === 'object'
            ? valueSchema?.description
            : undefined;
        return typeof schemaDescription === 'string' && schemaDescription.trim()
          ? schemaDescription
          : t('explorer.querySuggestions.attribute', {
              name: attributeLabel(suggestion.attribute),
            });
      }
      case 'relationship':
        return t('explorer.querySuggestions.relationship', {
          target: suggestion.target,
        });
      case 'allFields':
        return t('explorer.querySuggestions.allFields', {
          target: suggestion.target,
        });
      case 'global':
        return t('explorer.querySuggestions.global');
      case 'ids':
        return t('explorer.querySuggestions.ids');
      case 'value':
        return undefined;
    }
  };
  const typeLabel = (suggestion: QuerySuggestion) =>
    suggestion.kind === 'attribute'
      ? t(`explorer.queryValueTypes.${suggestion.attribute.value_type}`, {
          defaultValue: suggestion.attribute.value_type,
        })
      : suggestion.kind === 'relationship'
        ? t('explorer.querySuggestions.linksTo', { target: suggestion.target })
        : undefined;
  const errorMessage = (queryError: QueryError) =>
    t(`explorer.queryErrors.${queryError.code}`, { ...queryError });

  return (
    <QuerySegmentsContext.Provider value={segments}>
      <TextField
        error={Boolean(error)}
        fullWidth
        helperText={error && !errorContainer ? errorMessage(error) : undefined}
        inputRef={inputRef}
        label={t('explorer.query')}
        onBlur={() => {
          setFocused(false);
          setOpen(false);
        }}
        onChange={(event) => {
          onChange(event.target.value);
          trackCursor(event);
          setActive(-1);
          setOpen(true);
        }}
        onFocus={(event) => {
          setFocused(true);
          setOpen(true);
          trackCursor(event);
        }}
        placeholder={t('explorer.searchTerms')}
        ref={setAnchor}
        slotProps={{
          htmlInput: {
            'aria-activedescendant':
              showPopup && activeIndex >= 0 ? optionId(activeIndex) : undefined,
            'aria-autocomplete': 'list',
            ...(error && errorContainer && { 'aria-describedby': errorId }),
            'aria-controls': showPopup ? listboxId : undefined,
            'aria-expanded': showPopup,
            onKeyDown,
            onSelect: trackCursor,
            autoCapitalize: 'off',
            autoComplete: 'off',
            autoCorrect: 'off',
            role: 'combobox',
            spellCheck: false,
          },
          input: {
            inputComponent: HighlightedInput,
            endAdornment: (
              <InputAdornment position="end">
                <Tooltip title={t('explorer.searchSyntax')}>
                  <IconButton
                    aria-label={t('explorer.searchSyntax')}
                    onClick={(event) => onShowSyntax(event.currentTarget)}
                    size="small"
                    type="button"
                  >
                    <CircleQuestionMarkIcon size={smallIconSize} />
                  </IconButton>
                </Tooltip>
              </InputAdornment>
            ),
          },
        }}
        sx={{
          '& input::placeholder': { color: 'text.disabled', opacity: 1 },
          '& input': { caretColor: (theme) => theme.palette.text.primary },
        }}
        value={value}
      />
      {error &&
        errorContainer &&
        createPortal(
          <FormHelperText error id={errorId} sx={{ mx: 3.5 }}>
            {errorMessage(error)}
          </FormHelperText>,
          errorContainer,
        )}
      <Popper
        anchorEl={anchor}
        open={showPopup}
        placement="bottom-start"
        sx={(theme) => ({
          width: anchor?.clientWidth,
          zIndex: theme.zIndex.modal,
        })}
      >
        <Paper
          elevation={8}
          // Keep focus in the input while clicking suggestions.
          onMouseDown={(event) => event.preventDefault()}
          sx={{ mt: 1, overflow: 'hidden' }}
        >
          {valueHint !== undefined && (
            <Typography
              color="text.secondary"
              sx={{ px: 3, pt: 2, pb: suggestions.length ? 1 : 2 }}
              variant="body2"
            >
              {t('explorer.querySuggestions.valueHint', { field: valueHint })}
            </Typography>
          )}
          {loading && (
            <Stack
              direction="row"
              spacing={2}
              sx={{ alignItems: 'center', px: 3, py: 2 }}
            >
              <CircularProgress size={compactIconSize} />
              <Typography color="text.secondary" variant="body2">
                {t('explorer.loadingRelationshipFields')}
              </Typography>
            </Stack>
          )}
          {suggestions.length > 0 && (
            <Box
              aria-label={t('explorer.querySuggestions.label')}
              component="ul"
              id={listboxId}
              ref={listRef}
              role="listbox"
              sx={{
                listStyle: 'none',
                m: 0,
                maxHeight: querySuggestionListMaxHeight,
                overflowY: 'auto',
                p: 1,
              }}
            >
              {suggestions.map((suggestion, index) => {
                const Icon = suggestionIcon(suggestion);
                const description = describe(suggestion);
                const type = typeLabel(suggestion);
                return (
                  <Box
                    aria-selected={index === activeIndex}
                    component="li"
                    id={optionId(index)}
                    key={`${suggestion.kind}:${suggestion.code}`}
                    onClick={() => accept(suggestion)}
                    onMouseMove={() => {
                      if (index !== activeIndex) setActive(index);
                    }}
                    role="option"
                    sx={{
                      alignItems: 'flex-start',
                      borderRadius: 1,
                      cursor: 'pointer',
                      display: 'flex',
                      gap: 2,
                      px: 2,
                      py: 1.5,
                      ...(index === activeIndex && {
                        bgcolor: 'action.selected',
                      }),
                    }}
                  >
                    <Box
                      sx={{
                        color:
                          suggestion.kind === 'global'
                            ? 'secondary.main'
                            : 'primary.main',
                        display: 'flex',
                        pt: 0.5,
                      }}
                    >
                      <Icon size={compactIconSize} />
                    </Box>
                    <Box sx={{ flex: 1, minWidth: 0 }}>
                      <Stack
                        direction="row"
                        spacing={2}
                        sx={{
                          alignItems: 'baseline',
                          justifyContent: 'space-between',
                        }}
                      >
                        <Typography
                          component="span"
                          sx={{ fontWeight: 600, overflowWrap: 'anywhere' }}
                          variant="body2"
                        >
                          {suggestion.kind === 'allFields'
                            ? t('explorer.querySuggestions.allFieldsTitle', {
                                field: suggestion.code,
                              })
                            : suggestion.code}
                        </Typography>
                        {type && (
                          <Typography
                            color="text.secondary"
                            component="span"
                            sx={{ flexShrink: 0 }}
                            variant="caption"
                          >
                            {type}
                          </Typography>
                        )}
                      </Stack>
                      {description && (
                        <Typography
                          color="text.secondary"
                          component="span"
                          sx={{ display: 'block' }}
                          variant="caption"
                        >
                          {description}
                        </Typography>
                      )}
                    </Box>
                  </Box>
                );
              })}
            </Box>
          )}
          {suggestions.length > 0 && (
            <Typography
              color="text.secondary"
              component="div"
              sx={{
                borderColor: 'divider',
                borderTop: 1,
                display: { xs: 'none', sm: 'block' },
                px: 3,
                py: 1.5,
              }}
              variant="caption"
            >
              {t('explorer.querySuggestions.keyboardHint')}
            </Typography>
          )}
        </Paper>
      </Popper>
    </QuerySegmentsContext.Provider>
  );
};
