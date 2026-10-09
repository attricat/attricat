import { LogOutIcon, MoonIcon, SunIcon, type LucideIcon } from 'lucide-react';
import { LazyMotion, domAnimation, type Variants } from 'motion/react';
import * as m from 'motion/react-m';
import { createElement, useRef, type ReactNode } from 'react';
import {
  gesture,
  navigationIconStagger,
  useNavigationIconActive,
} from './navigationIconMotion';
import {
  AgentIcon,
  AppsIcon,
  AuditLogIcon,
  BackgroundProcessingIcon,
  BlueprintIcon,
  ContextIcon,
  DataHealthIcon,
  DocumentationIcon,
  ExplorerIcon,
  ExplorerShortcutIcon,
  ExportIcon,
  ExtensionIcon,
  InboxIcon,
  LexiconIcon,
  ManagementIcon,
  ProfileIcon,
  ReusableAttributeIcon,
  RuleIcon,
  SystemHealthIcon,
  WorkflowIcon,
  WorkspaceIcon,
} from './systemIcons';

// Navigation-only animated counterparts of registry icons. Geometry matches
// the installed lucide-react glyphs; several gestures are adapted from
// lucide-animated (MIT, https://lucide-animated.com). Motion transforms SVG
// parts around their own bounding box, so `originX`/`originY` are fractions
// of that box.

const lift = (order = 0) =>
  gesture({ y: [0, -2, 0] }, { y: 0 }, order * navigationIconStagger);
const nudge = (x: number, y = 0) =>
  gesture({ x: [0, x, 0], y: [0, y, 0] }, { x: 0, y: 0 });
const pulse = (order = 0) =>
  gesture({ scale: [1, 0.8, 1] }, { scale: 1 }, order * navigationIconStagger);
const draw = (order = 0) =>
  gesture(
    { pathLength: [0, 1] },
    { pathLength: 1 },
    order * navigationIconStagger,
  );
const turn = (rotate: number) => gesture({ rotate }, { rotate: 0 });
const wiggle = (degrees: number) =>
  gesture(
    { rotate: [0, -degrees, degrees * 0.8, -degrees * 0.4, 0] },
    { rotate: 0 },
  );
const blink = gesture({ scaleY: [1, 0.1, 1] }, { scaleY: 1 });

type AnimatedIcon = { body: ReactNode; svg?: Variants };

const animatedNavigationIcons = new Map<LucideIcon, AnimatedIcon>([
  // The eyes blink.
  [
    AgentIcon,
    {
      body: (
        <>
          <path d="M12 8V4H8" />
          <rect height="12" rx="2" width="16" x="4" y="8" />
          <path d="M2 14h2" />
          <path d="M20 14h2" />
          <m.path d="M15 13v2" variants={blink} />
          <m.path d="M9 13v2" variants={blink} />
        </>
      ),
    },
  ],
  // The tiles pulse in turn, clockwise.
  [
    AppsIcon,
    {
      body: (
        <>
          <m.rect height="7" rx="1" variants={pulse(0)} width="7" x="3" y="3" />
          <m.rect
            height="7"
            rx="1"
            variants={pulse(1)}
            width="7"
            x="14"
            y="3"
          />
          <m.rect
            height="7"
            rx="1"
            variants={pulse(2)}
            width="7"
            x="14"
            y="14"
          />
          <m.rect
            height="7"
            rx="1"
            variants={pulse(3)}
            width="7"
            x="3"
            y="14"
          />
        </>
      ),
    },
  ],
  // The check is drawn.
  [
    AuditLogIcon,
    {
      body: (
        <>
          <rect height="4" rx="1" ry="1" width="8" x="8" y="2" />
          <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
          <m.path d="m9 14 2 2 4-4" variants={draw()} />
        </>
      ),
    },
  ],
  // The clock hand sweeps a full turn about the clock's centre (16, 16).
  [
    BackgroundProcessingIcon,
    {
      body: (
        <>
          <m.path
            d="M16 14v2.2l1.6 1"
            style={{ originX: 0, originY: 0.625 }}
            variants={turn(360)}
          />
          <path d="M16 4h2a2 2 0 0 1 2 2v.832" />
          <path d="M8 4H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h2" />
          <circle cx="16" cy="16" r="6" />
          <rect height="4" rx="1" width="8" x="8" y="2" />
        </>
      ),
    },
  ],
  // The shapes lift in turn, like parts of a blueprint being assembled.
  [
    BlueprintIcon,
    {
      body: (
        <>
          <m.path
            d="M8.3 10a.7.7 0 0 1-.626-1.079L11.4 3a.7.7 0 0 1 1.198-.043L16.3 8.9a.7.7 0 0 1-.572 1.1Z"
            variants={lift(0)}
          />
          <m.rect height="7" rx="1" variants={lift(1)} width="7" x="3" y="14" />
          <m.circle cx="17.5" cy="17.5" r="3.5" variants={lift(2)} />
        </>
      ),
    },
  ],
  // The folder tips open from its base.
  [
    ContextIcon,
    {
      body: (
        <m.path
          d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"
          style={{ originX: 0, originY: 1 }}
          variants={gesture({ rotate: [0, -8, 0] }, { rotate: 0 })}
        />
      ),
    },
  ],
  // The columns are drawn up from the axis in turn.
  [
    DataHealthIcon,
    {
      body: (
        <>
          <path d="M3 3v16a2 2 0 0 0 2 2h16" />
          <m.path d="M8 17v-3" variants={draw(0)} />
          <m.path d="M13 17V5" variants={draw(1)} />
          <m.path d="M18 17V9" variants={draw(2)} />
        </>
      ),
    },
  ],
  // The pages flex from the spine.
  [
    DocumentationIcon,
    {
      body: (
        <>
          <path d="M12 5v16" />
          <path d="M20.001 19A2 2 0 0022 17V5a2 2 0 00-1.999-2L16 3.002A5 5 0 0012 5a5 5 0 00-4-2H4a2 2 0 00-2 2v12a2 2 0 001.999 2H8a5 5 0 014 2 5 5 0 014-2z" />
        </>
      ),
      svg: gesture({ scaleX: [1, 0.85, 1] }, { scaleX: 1 }),
    },
  ],
  // The needle turns a full circle about its own centre.
  [
    ExplorerIcon,
    {
      body: (
        <>
          <circle cx="12" cy="12" r="10" />
          <m.path
            d="m16.24 7.76-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z"
            variants={turn(360)}
          />
        </>
      ),
    },
  ],
  // The ribbon stretches down from its top edge.
  [
    ExplorerShortcutIcon,
    {
      body: (
        <m.path
          d="M17 3a2 2 0 0 1 2 2v15a1 1 0 0 1-1.496.868l-4.512-2.578a2 2 0 0 0-1.984 0l-4.512 2.578A1 1 0 0 1 5 20V5a2 2 0 0 1 2-2z"
          style={{ originY: 0 }}
          variants={gesture({ scaleY: [1, 1.12, 1] }, { scaleY: 1 })}
        />
      ),
    },
  ],
  // The arrow pushes out of the file.
  [
    ExportIcon,
    {
      body: (
        <>
          <path d="M4.226 20.925A2 2 0 0 0 6 22h12a2 2 0 0 0 2-2V8a2.4 2.4 0 0 0-.706-1.706l-3.588-3.588A2.4 2.4 0 0 0 14 2H6a2 2 0 0 0-2 2v3.127" />
          <path d="M14 2v5a1 1 0 0 0 1 1h5" />
          <m.g variants={nudge(-2)}>
            <path d="m5 11-3 3" />
            <path d="m5 17-3-3h10" />
          </m.g>
        </>
      ),
    },
  ],
  // The piece wiggles as if being fitted.
  [
    ExtensionIcon,
    {
      body: (
        <m.path
          d="M15.39 4.39a1 1 0 0 0 1.68-.474 2.5 2.5 0 1 1 3.014 3.015 1 1 0 0 0-.474 1.68l1.683 1.682a2.414 2.414 0 0 1 0 3.414L19.61 15.39a1 1 0 0 1-1.68-.474 2.5 2.5 0 1 0-3.014 3.015 1 1 0 0 1 .474 1.68l-1.683 1.682a2.414 2.414 0 0 1-3.414 0L8.61 19.61a1 1 0 0 0-1.68.474 2.5 2.5 0 1 1-3.014-3.015 1 1 0 0 0 .474-1.68l-1.683-1.682a2.414 2.414 0 0 1 0-3.414L4.39 8.61a1 1 0 0 1 1.68.474 2.5 2.5 0 1 0 3.014-3.015 1 1 0 0 1-.474-1.68l1.683-1.682a2.414 2.414 0 0 1 3.414 0z"
          variants={wiggle(10)}
        />
      ),
    },
  ],
  // The bell swings from its top.
  [
    InboxIcon,
    {
      body: (
        <m.g style={{ originY: 0 }} variants={wiggle(14)}>
          <path d="M10.268 21a2 2 0 0 0 3.464 0" />
          <path d="M3.262 15.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673C19.41 13.956 18 12.499 18 8A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326" />
        </m.g>
      ),
    },
  ],
  // The Latin "A" is written.
  [
    LexiconIcon,
    {
      body: (
        <>
          <path d="m5 8 6 6" />
          <path d="m4 14 6-6 2-3" />
          <path d="M2 5h12" />
          <path d="M7 2h1" />
          <m.path d="m22 22-5-10-5 10" variants={draw(0)} />
          <m.path d="M14 18h6" variants={draw(3)} />
        </>
      ),
    },
  ],
  // The arrow steps out of the door.
  [
    LogOutIcon,
    {
      body: (
        <>
          <m.g variants={nudge(2)}>
            <path d="m16 17 5-5-5-5" />
            <path d="M21 12H9" />
          </m.g>
          <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
        </>
      ),
    },
  ],
  // The gear has six-fold symmetry, so a 60° turn ends on its resting pose.
  [
    ManagementIcon,
    {
      body: (
        <>
          <path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915" />
          <circle cx="12" cy="12" r="3" />
        </>
      ),
      svg: turn(60),
    },
  ],
  // The moon rocks.
  [
    MoonIcon,
    {
      body: (
        <path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401" />
      ),
      svg: wiggle(12),
    },
  ],
  // The head nods.
  [
    ProfileIcon,
    {
      body: (
        <>
          <path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" />
          <m.circle cx="12" cy="7" r="4" variants={lift()} />
        </>
      ),
    },
  ],
  // The loose block steps out and clicks back into place.
  [
    ReusableAttributeIcon,
    {
      body: (
        <>
          <path d="M10 22V7a1 1 0 0 0-1-1H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-5a1 1 0 0 0-1-1H2" />
          <m.rect
            height="8"
            rx="1"
            variants={nudge(1.5, -1.5)}
            width="8"
            x="14"
            y="2"
          />
        </>
      ),
    },
  ],
  // The check is drawn.
  [
    RuleIcon,
    {
      body: (
        <>
          <path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" />
          <m.path d="m9 12 2 2 4-4" variants={draw()} />
        </>
      ),
    },
  ],
  // The eight rays turn by one step, ending on the resting pose.
  [
    SunIcon,
    {
      body: (
        <>
          <circle cx="12" cy="12" r="4" />
          <path d="M12 2v2" />
          <path d="M12 20v2" />
          <path d="m4.93 4.93 1.41 1.41" />
          <path d="m17.66 17.66 1.41 1.41" />
          <path d="M2 12h2" />
          <path d="M20 12h2" />
          <path d="m6.34 17.66-1.41 1.41" />
          <path d="m19.07 4.93-1.41 1.41" />
        </>
      ),
      svg: turn(45),
    },
  ],
  // The pulse line is traced.
  [
    SystemHealthIcon,
    {
      body: (
        <m.path
          d="M22 12h-2.48a2 2 0 0 0-1.93 1.46l-2.35 8.36a.25.25 0 0 1-.48 0L9.24 2.18a.25.25 0 0 0-.48 0l-2.35 8.36A2 2 0 0 1 4.49 12H2"
          variants={draw()}
        />
      ),
    },
  ],
  // The connector is drawn, then the next step responds.
  [
    WorkflowIcon,
    {
      body: (
        <>
          <rect height="8" rx="2" width="8" x="3" y="3" />
          <m.path d="M7 11v4a2 2 0 0 0 2 2h4" variants={draw()} />
          <m.rect
            height="8"
            rx="2"
            variants={pulse(4)}
            width="8"
            x="13"
            y="13"
          />
        </>
      ),
    },
  ],
  // The cog's eight teeth turn by one step, ending on the resting pose.
  [
    WorkspaceIcon,
    {
      body: (
        <>
          <path d="M10 15H6a4 4 0 0 0-4 4v2" />
          <m.g variants={turn(45)}>
            <path d="m14.305 16.53.923-.382" />
            <path d="m15.228 13.852-.923-.383" />
            <path d="m16.852 12.228-.383-.923" />
            <path d="m16.852 17.772-.383.924" />
            <path d="m19.148 12.228.383-.923" />
            <path d="m19.53 18.696-.382-.924" />
            <path d="m20.772 13.852.924-.383" />
            <path d="m20.772 16.148.924.383" />
            <circle cx="18" cy="15" r="3" />
          </m.g>
          <circle cx="9" cy="7" r="4" />
        </>
      ),
    },
  ],
]);

/**
 * A registry icon for the side navigation. It plays a short gesture while its
 * row is hovered or keyboard-focused; icons without one render statically.
 */
export const NavigationIcon = ({ icon }: { icon: LucideIcon }) => {
  const ref = useRef<SVGSVGElement>(null);
  const active = useNavigationIconActive(ref);
  const animated = animatedNavigationIcons.get(icon);
  if (!animated) return createElement(icon);
  return (
    <LazyMotion features={domAnimation} strict>
      <m.svg
        animate={active ? 'active' : 'rest'}
        aria-hidden="true"
        className="lucide"
        fill="none"
        height="1em"
        initial={false}
        ref={ref}
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2}
        variants={animated.svg}
        viewBox="0 0 24 24"
        width="1em"
        xmlns="http://www.w3.org/2000/svg"
      >
        {animated.body}
      </m.svg>
    </LazyMotion>
  );
};
