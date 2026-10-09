import { Box } from '@mui/material';
import { LazyMotion, domAnimation } from 'motion/react';
import * as m from 'motion/react-m';
import { useRef } from 'react';
import {
  gesture,
  navigationIconStagger,
  useNavigationIconActive,
} from './navigationIconMotion';

// The navigation header's Attricat mark and wordmark, drawn inline so the
// boxes can stack when the header is hovered. Geometry matches
// design/assets/logos/{mark,wordmark-*}.svg; colors use the theme's brand
// primary and primary text, as those assets do.

const stack = (order: number) =>
  gesture({ y: [0, -3, 0] }, { y: 0 }, order * navigationIconStagger);

const brandIconHeight = 32;
const brandIconWidths = { mark: 32, wordmark: 161 } as const;

export const NavigationBrandIcon = ({
  variant,
}: {
  variant: 'mark' | 'wordmark';
}) => {
  const ref = useRef<SVGSVGElement>(null);
  const active = useNavigationIconActive(ref);
  return (
    <LazyMotion features={domAnimation} strict>
      <Box
        animate={active ? 'active' : 'rest'}
        aria-hidden="true"
        component={m.svg}
        fill="none"
        initial={false}
        ref={ref}
        sx={{
          display: 'block',
          height: brandIconHeight,
          width: brandIconWidths[variant],
        }}
        viewBox={variant === 'mark' ? '0 0 48 48' : '0 0 242 48'}
      >
        <Box
          component="g"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          sx={{ stroke: (theme) => theme.palette.primary.main }}
          transform="translate(6 6) scale(1.5)"
        >
          <m.g variants={stack(0)}>
            <path d="M2.97 12.92A2 2 0 0 0 2 14.63v3.24a2 2 0 0 0 .97 1.71l3 1.8a2 2 0 0 0 2.06 0L12 19v-5.5l-5-3-4.03 2.42Z" />
            <path d="m7 16.5-4.74-2.85" />
            <path d="m7 16.5 5-3" />
            <path d="M7 16.5v5.17" />
          </m.g>
          <m.g variants={stack(1)}>
            <path d="M12 13.5V19l3.97 2.38a2 2 0 0 0 2.06 0l3-1.8a2 2 0 0 0 .97-1.71v-3.24a2 2 0 0 0-.97-1.71L17 10.5l-5 3Z" />
            <path d="m17 16.5-5-3" />
            <path d="m17 16.5 4.74-2.85" />
            <path d="M17 16.5v5.17" />
          </m.g>
          <m.g variants={stack(2)}>
            <path d="M7.97 4.42A2 2 0 0 0 7 6.13v4.37l5 3 5-3V6.13a2 2 0 0 0-.97-1.71l-3-1.8a2 2 0 0 0-2.06 0l-3 1.8Z" />
            <path d="M12 8 7.26 5.15" />
            <path d="m12 8 4.74-2.85" />
            <path d="M12 13.5V8" />
          </m.g>
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
    </LazyMotion>
  );
};
