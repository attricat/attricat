import type { LucideIcon } from 'lucide-react';
import { createElement, lazy, Suspense } from 'react';

// Motion and the gesture definitions load in their own chunk, so the initial
// bundle only carries this wrapper.
const AnimatedNavigationIcon = lazy(() => import('./AnimatedNavigationIcon'));

/**
 * A registry icon for the side navigation. It plays a short gesture while its
 * row is hovered or keyboard-focused; until the gestures load, and for icons
 * without one, it renders the static Lucide icon.
 */
export const NavigationIcon = ({ icon }: { icon: LucideIcon }) => (
  <Suspense fallback={createElement(icon)}>
    <AnimatedNavigationIcon icon={icon} />
  </Suspense>
);
