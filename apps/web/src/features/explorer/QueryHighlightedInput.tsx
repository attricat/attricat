import { Box, type InputBaseComponentProps } from '@mui/material';
import {
  createContext,
  useContext,
  useLayoutEffect,
  useRef,
  type ReactNode,
  type Ref,
} from 'react';
import type { QuerySegment } from './queryLanguage';

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
export const HighlightedInput = ({
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
  // Reading the input's scroll position forces a layout, so it is synced only
  // when the query text changes; scrolling and selection sync it below.
  useLayoutEffect(syncScroll, [props.value]);
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

/** Supplies the highlighted segments `HighlightedInput` draws. */
export const QuerySegmentsProvider = ({
  children,
  segments,
}: {
  children: ReactNode;
  segments: QuerySegment[];
}) => (
  <QuerySegmentsContext.Provider value={segments}>
    {children}
  </QuerySegmentsContext.Provider>
);
