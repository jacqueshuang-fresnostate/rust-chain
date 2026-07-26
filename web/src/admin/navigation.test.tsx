import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';

import { adminNavItems } from './navigation';
import { adminRoutes } from './routes';

const navPaths = adminNavItems
  .flatMap((item) => [item.path, ...(item.children?.map((child) => child.path) ?? [])])
  .filter((path): path is string => Boolean(path));

const routePaths = new Set(
  adminRoutes.flatMap((route) => (typeof route.path === 'string' ? [`/admin/${route.path}`] : []))
);

describe('admin navigation registry', () => {
  it('only references registered admin routes', () => {
    const unknown = navPaths.filter((path) => !routePaths.has(path));
    expect(unknown).toEqual([]);
  });

  it('never lists the same path twice', () => {
    expect(new Set(navPaths).size).toBe(navPaths.length);
  });

  it('gives every nav group a distinct icon', () => {
    const iconTypes = adminNavItems.map((item) => (item.icon as ReactElement | undefined)?.type);
    expect(iconTypes.every(Boolean)).toBe(true);
    expect(new Set(iconTypes).size).toBe(iconTypes.length);
  });
});
