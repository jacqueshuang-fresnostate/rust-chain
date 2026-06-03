import { isValidElement } from 'react';
import { describe, expect, it } from 'vitest';

import { adminRoutes } from './routes';

function routeElementName(path: string) {
  const route = adminRoutes.find((candidate) => candidate.path === path);
  const element = route?.element;
  return isValidElement(element) && typeof element.type !== 'string' ? String(element.type.name ?? '') : '';
}

describe('adminRoutes', () => {
  it.each([
    'new-coins/subscriptions',
    'new-coins/distributions',
    'users',
    'assets',
    'wallet/accounts',
    'wallet/ledger',
    'risk',
    'risk/events'
  ])('uses resource page for %s', (path) => {
    expect(routeElementName(path)).toBe('ResourcePage');
  });

  it('registers the market feed configuration action page', () => {
    expect(routeElementName('market/feed-config')).toBe('MarketFeedConfigPage');
  });

  it('registers the SMTP configuration action page', () => {
    expect(routeElementName('system/smtp')).toBe('SmtpConfigPage');
  });

  it.each(['spot/actions', 'seconds-contract/actions', 'margin/actions'])('keeps existing product action route %s', (path) => {
    expect(routeElementName(path)).toBe('ProductStatusActions');
  });
});
