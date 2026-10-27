import type { BrandVariant } from './brandIconSizes';
import { LazyMotion, domAnimation, useReducedMotion } from 'motion/react';
import * as m from 'motion/react-m';
import { useRef } from 'react';
import { apiHealthStates } from '../features/inspector/apiHealth';
import { useApiHealth } from '../features/inspector/useApiHealth';
import { NavigationBrandGraphic } from './NavigationBrandIcon';
import {
  gesture,
  navigationIconStagger,
  navigationIconTransition,
  useNavigationIconActive,
} from './navigationIconMotion';

const stackLift = { y: [0, -3, 0] };
const stackBoxCount = 3;
/** Pause between rounds of the restarting loop, after the last box lands. */
const restartingLoopPause = stackBoxCount * navigationIconStagger;
const restartingVariant = 'restarting';

const stack = (order: number) => ({
  ...gesture(stackLift, { y: 0 }, order * navigationIconStagger),
  [restartingVariant]: {
    ...stackLift,
    transition: {
      ...navigationIconTransition,
      delay: order * navigationIconStagger,
      repeat: Infinity,
      repeatDelay: restartingLoopPause,
    },
  },
});

// Loaded lazily by `NavigationBrandIcon`: the mark's boxes stack in turn while
// the navigation header is hovered, and keep stacking while the Inspector
// reports the API restarting.
const AnimatedNavigationBrandIcon = ({
  variant,
}: {
  variant: BrandVariant;
}) => {
  const ref = useRef<SVGSVGElement>(null);
  const active = useNavigationIconActive(ref);
  const reducedMotion = useReducedMotion();
  const apiHealth = useApiHealth();
  const restarting = !reducedMotion && apiHealth === apiHealthStates.restarting;
  return (
    <LazyMotion features={domAnimation} strict>
      <NavigationBrandGraphic
        box={(order, paths) => <m.g variants={stack(order)}>{paths}</m.g>}
        svg={m.svg}
        svgProps={{
          animate: restarting ? restartingVariant : active ? 'active' : 'rest',
          initial: false,
          ref,
        }}
        variant={variant}
      />
    </LazyMotion>
  );
};

export default AnimatedNavigationBrandIcon;
