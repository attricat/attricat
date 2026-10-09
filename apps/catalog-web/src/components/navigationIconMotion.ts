import motionTokens from '../../design/tokens/motion.json';
import type { TargetAndTransition, Variants } from 'motion/react';
import { useReducedMotion } from 'motion/react';
import { useEffect, useState, type RefObject } from 'react';

// Shared timing for navigation icon gestures, from the design motion tokens
// (milliseconds there, seconds in Motion). Each gesture plays once when its row
// is hovered or keyboard-focused and ends on the icon's resting pose, so
// leaving the row snaps back without a second animation.
export const navigationIconTransition = {
  duration: motionTokens.duration.gesture / 1000,
  ease: motionTokens.easing.standard as [number, number, number, number],
};

/** Delay between parts of one icon that move in turn. */
export const navigationIconStagger = motionTokens.stagger / 1000;

/** Element that triggers the gestures of the navigation icons inside it. */
export const navigationIconTriggerSelector =
  '.MuiListItemButton-root, [data-navigation-icon-trigger]';

/**
 * Variants for one moving part: `active` plays the gesture, `rest` returns to
 * the resting pose instantly.
 */
export const gesture = (
  active: TargetAndTransition,
  rest: TargetAndTransition,
  delay = 0,
): Variants => ({
  active: { ...active, transition: { ...navigationIconTransition, delay } },
  rest: { ...rest, transition: { duration: 0 } },
});

/**
 * Whether the navigation row containing `ref` is hovered or keyboard-focused.
 * Pointer focus (a click) does not replay the gesture, and reduced-motion
 * preferences disable it.
 */
export const useNavigationIconActive = (ref: RefObject<Element | null>) => {
  const reducedMotion = useReducedMotion();
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);

  useEffect(() => {
    const trigger = ref.current?.closest(navigationIconTriggerSelector);
    if (!trigger || reducedMotion) return;
    const enter = () => setHovered(true);
    const leave = () => setHovered(false);
    const focus = () => setFocused(trigger.matches(':focus-visible'));
    const blur = () => setFocused(false);
    trigger.addEventListener('pointerenter', enter);
    trigger.addEventListener('pointerleave', leave);
    trigger.addEventListener('focus', focus);
    trigger.addEventListener('blur', blur);
    return () => {
      trigger.removeEventListener('pointerenter', enter);
      trigger.removeEventListener('pointerleave', leave);
      trigger.removeEventListener('focus', focus);
      trigger.removeEventListener('blur', blur);
    };
  }, [ref, reducedMotion]);

  return !reducedMotion && (hovered || focused);
};
