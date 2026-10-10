import { LazyMotion, domAnimation } from 'motion/react';
import * as m from 'motion/react-m';
import { useRef } from 'react';
import { NavigationBrandGraphic } from './NavigationBrandIcon';
import {
  gesture,
  navigationIconStagger,
  useNavigationIconActive,
} from './navigationIconMotion';

const stack = (order: number) =>
  gesture({ y: [0, -3, 0] }, { y: 0 }, order * navigationIconStagger);

// Loaded lazily by `NavigationBrandIcon`: the mark's boxes stack in turn while
// the navigation header is hovered.
const AnimatedNavigationBrandIcon = ({
  variant,
}: {
  variant: 'mark' | 'wordmark';
}) => {
  const ref = useRef<SVGSVGElement>(null);
  const active = useNavigationIconActive(ref);
  return (
    <LazyMotion features={domAnimation} strict>
      <NavigationBrandGraphic
        box={(order, paths) => <m.g variants={stack(order)}>{paths}</m.g>}
        svg={m.svg}
        svgProps={{
          animate: active ? 'active' : 'rest',
          initial: false,
          ref,
        }}
        variant={variant}
      />
    </LazyMotion>
  );
};

export default AnimatedNavigationBrandIcon;
