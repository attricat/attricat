import type { ComponentType } from 'react';

type LinkProps = { to: string; params?: Record<string, string> };

const href = ({ to, params = {} }: LinkProps) =>
  to.replace(/\$(\w+)/g, (_, name: string) => params[name] ?? '');

/**
 * Stands in for `@tanstack/react-router` where a component renders router
 * links but needs no navigation: each link becomes an anchor to its path.
 */
export const routerLinkMock = {
  createLink:
    <P extends object>(Component: ComponentType<P & { href: string }>) =>
    ({ to, params, ...props }: P & LinkProps) => (
      <Component {...(props as P)} href={href({ to, params })} />
    ),
};
