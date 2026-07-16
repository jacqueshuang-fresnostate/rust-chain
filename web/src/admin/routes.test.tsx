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
    'news',
    'system/countries',
    'new-coins/subscriptions',
    'new-coins/distributions',
    'users',
    'agent-commission-rules',
    'assets',
    'wallet/accounts',
    'wallet/deposit-network-configs',
    'wallet/deposit-address-pool',
    'wallet/quick-recharge-orders',
    'wallet/ledger',
    'loan/products',
    'loan/orders',
    'earn/categories',
    'risk',
    'risk/events'
  ])('uses resource page for %s', (path) => {
    expect(routeElementName(path)).toBe('ResourcePage');
  });

  it('registers the market feed configuration action page', () => {
    expect(routeElementName('market/feed-config')).toBe('MarketFeedConfigPage');
  });

  it('registers the KYC management action page', () => {
    expect(routeElementName('users/kyc')).toBe('KycManagementPage');
  });

  it('registers the SMTP configuration action page', () => {
    expect(routeElementName('system/smtp')).toBe('SmtpConfigPage');
  });

  it('registers the upload configuration action page', () => {
    expect(routeElementName('system/uploads')).toBe('UploadConfigPage');
  });

  it('registers the PC brand configuration action page', () => {
    expect(routeElementName('system/brand')).toBe('PlatformBrandPage');
  });

  it('registers the quick recharge configuration action page', () => {
    expect(routeElementName('wallet/quick-recharge')).toBe('QuickRechargeConfigPage');
  });

  it('registers the security policy action page', () => {
    expect(routeElementName('system/security-policy')).toBe('SecurityPolicyPage');
  });

  it('does not register the removed margin product action route', () => {
    expect(routeElementName('margin/actions')).toBe('');
  });

  it('does not register the removed spot product action route', () => {
    expect(routeElementName('spot/actions')).toBe('');
  });

  it('does not register a duplicate seconds contract action route', () => {
    expect(routeElementName('seconds-contract/actions')).toBe('');
  });

  it('does not register the removed Earn product action route', () => {
    expect(routeElementName('earn/actions')).toBe('');
  });

  it('does not register the removed new coin convert rule page', () => {
    expect(routeElementName('convert/rules')).toBe('');
  });
});
