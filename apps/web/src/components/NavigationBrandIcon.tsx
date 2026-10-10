import { Box } from '@mui/material';
import { lazy, Suspense, type ElementType, type ReactNode } from 'react';

// The navigation header's Attricat mark and wordmark, drawn inline so the
// boxes can stack when the header is hovered. Geometry matches
// design/assets/logos/{mark,wordmark-*}.svg; colors use the theme's brand
// primary and primary text, as those assets do.

const brandIconHeight = 32;
const brandIconWidths = { mark: 32, wordmark: 161 } as const;

type BrandVariant = 'mark' | 'wordmark';

type NavigationBrandGraphicProps = {
  /** Renders one of the mark's three boxes, numbered bottom-left first. */
  box?: (order: number, paths: ReactNode) => ReactNode;
  /** Root SVG element and its extra props, for the animated variant. */
  svg?: ElementType;
  svgProps?: Record<string, unknown>;
  variant: BrandVariant;
};

export const NavigationBrandGraphic = ({
  box = (_, paths) => <g>{paths}</g>,
  svg = 'svg',
  svgProps,
  variant,
}: NavigationBrandGraphicProps) => (
  <Box
    aria-hidden="true"
    component={svg}
    fill="none"
    sx={{
      display: 'block',
      height: brandIconHeight,
      width: brandIconWidths[variant],
    }}
    viewBox={variant === 'mark' ? '0 0 48 48' : '0 0 242 48'}
    {...svgProps}
  >
    <Box
      component="g"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={2}
      sx={{ stroke: (theme) => theme.palette.primary.main }}
      transform="translate(6 6) scale(1.5)"
    >
      {box(
        0,
        <>
          <path d="M2.97 12.92A2 2 0 0 0 2 14.63v3.24a2 2 0 0 0 .97 1.71l3 1.8a2 2 0 0 0 2.06 0L12 19v-5.5l-5-3-4.03 2.42Z" />
          <path d="m7 16.5-4.74-2.85" />
          <path d="m7 16.5 5-3" />
          <path d="M7 16.5v5.17" />
        </>,
      )}
      {box(
        1,
        <>
          <path d="M12 13.5V19l3.97 2.38a2 2 0 0 0 2.06 0l3-1.8a2 2 0 0 0 .97-1.71v-3.24a2 2 0 0 0-.97-1.71L17 10.5l-5 3Z" />
          <path d="m17 16.5-5-3" />
          <path d="m17 16.5 4.74-2.85" />
          <path d="M17 16.5v5.17" />
        </>,
      )}
      {box(
        2,
        <>
          <path d="M7.97 4.42A2 2 0 0 0 7 6.13v4.37l5 3 5-3V6.13a2 2 0 0 0-.97-1.71l-3-1.8a2 2 0 0 0-2.06 0l-3 1.8Z" />
          <path d="M12 8 7.26 5.15" />
          <path d="m12 8 4.74-2.85" />
          <path d="M12 13.5V8" />
        </>,
      )}
    </Box>
    {variant === 'wordmark' && (
      <Box
        component="text"
        fontFamily="Inter,Arial,sans-serif"
        fontSize={30}
        fontWeight={600}
        letterSpacing={-1.5}
        sx={{ fill: (theme) => theme.palette.text.primary }}
        x={54}
        y={33}
      >
        attricat
      </Box>
    )}
  </Box>
);

// Motion loads in its own chunk; the static graphic shows until then.
const AnimatedNavigationBrandIcon = lazy(
  () => import('./AnimatedNavigationBrandIcon'),
);

export const NavigationBrandIcon = ({ variant }: { variant: BrandVariant }) => (
  <Suspense fallback={<NavigationBrandGraphic variant={variant} />}>
    <AnimatedNavigationBrandIcon variant={variant} />
  </Suspense>
);
