import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { adminNavItems, type AdminNavItem } from '../admin/navigation';
import { toCsv } from '../admin/resources/AdminResourcePage';
import { resourceConfigs } from '../admin/resources/resourceConfigs';
import { buildAuditFieldChanges } from '../admin/audit/auditPresentation';
import { ADMIN_FIELD_VALUE_LABELS } from './adminEnumLabels';
import { adminEnumLabel, adminFieldLabel } from './adminPresentation';
import { displayDetailValue } from './DetailDrawer';
import { StatusTag } from './StatusTag';

describe('Admin Chinese presentation coverage', () => {
  it('does not apply shared display translations to raw CSV enum or JSON values', () => {
    const csv = toCsv([{ key: 'scenario', title: '场景' }, { key: 'generator', title: '生成模型', type: 'json' }], [{ scenario: 'trend_up', generator: { seed_mode: 'fixed', seed: 'active' } }]);
    expect(csv).toContain('trend_up');
    expect(csv).toContain('""seed_mode"":""fixed""');
    expect(csv).toContain('""seed"":""active""');
    expect(csv).not.toContain('稳步上涨');
    const explicitMapCsv = toCsv([{ key: 'status', title: '状态', valueMap: { active: '启用' } }], [{ status: 'active' }, { status: 'constructor' }]);
    expect(explicitMapCsv).toBe('状态\n启用\nconstructor');
  });
  it('covers every registered navigation category and API resource column with readable labels', () => {
    const visit = (items: AdminNavItem[]): number => items.reduce((count, item) => {
      expect(item.label).toMatch(/[\u3400-\u9fff]/u);
      return count + (item.path ? 1 : 0) + visit(item.children ?? []);
    }, 0);
    expect(visit(adminNavItems)).toBeGreaterThan(40);
    for (const [name, config] of Object.entries(resourceConfigs)) {
      expect(config.title, name).toMatch(/[\u3400-\u9fff]/u);
      for (const column of config.columns) {
        if (!('source' in column) || column.source !== 'derived') expect(adminFieldLabel(column.key), `${name}.${column.key}`).not.toBe(column.key);
      }
    }
  });

  it('maps all declared enum domains without changing the wire values', () => {
    for (const [field, labels] of Object.entries(ADMIN_FIELD_VALUE_LABELS)) {
      for (const [raw, label] of Object.entries(labels)) {
        expect(adminEnumLabel(field, raw), `${field}.${raw}`).toBe(label);
        expect(displayDetailValue(raw, field)).toBe(label);
      }
    }
    expect(adminEnumLabel('role_name', 'SUPER_ADMIN')).toBe('超级管理员');
    expect(adminEnumLabel('role_name', 'custom-role')).toBeNull();
    expect(adminEnumLabel('margin_modes', 'cross')).toBe('全仓');
  });

  it.each(['paused', 'stopped', 'running', 'live', 'partial_allocated', 'refunded', 'pending_confirmation'])('uses consistent labels for %s across status and details', (value) => {
    render(<StatusTag value={value} />);
    expect(screen.getByText(displayDetailValue(value, 'status'))).toBeInTheDocument();
  });

  it('preserves free text, custom values and unknown keys, including object prototype names', () => {
    for (const key of ['name', 'title', 'username', 'seed', 'address', 'symbol', 'reason', 'constructor', '__proto__']) {
      expect(adminEnumLabel(key, 'active')).toBeNull();
      expect(displayDetailValue('active', key)).toBe('active');
    }
    expect(adminFieldLabel('unknown_future_field')).toBe('unknown_future_field');
    expect(adminFieldLabel('constructor')).toBe('constructor');
    expect(adminEnumLabel('status', 'constructor')).toBeNull();
    expect(displayDetailValue('future-code', 'status')).toBe('future-code');
    expect(displayDetailValue('active', 'custom_status', { types: { custom_status: 'status' } })).toBe('启用');
    expect(displayDetailValue('closed', 'status', { valueMaps: { status: { closed: '已关闭工单' } } })).toBe('已关闭工单');
  });

  it('localizes audit field/status changes without translating ordinary content or exposing secrets', () => {
    const changes = buildAuditFieldChanges({ username: 'pending', status: 'active', generator: { scenario: 'range', seed: 'active' }, password: 'old-password' }, { username: 'active', status: 'paused', generator: { scenario: 'trend_up', seed: 'pending' }, password: 'new-password' });
    expect(changes).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: '用户名', before: 'pending', after: 'active' }),
      expect.objectContaining({ label: '状态', before: '启用', after: '暂停' }),
      expect.objectContaining({ before: '区间震荡', after: '稳步上涨' })
    ]));
    expect(JSON.stringify(changes)).not.toMatch(/old-password|new-password/);
    const unknownChanges = buildAuditFieldChanges({ constructor: 'before', environment: 'production' }, { constructor: 'after', environment: 'constructor' });
    expect(unknownChanges).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: '字段「constructor」', before: 'before', after: 'after' }),
      expect.objectContaining({ after: 'constructor' })
    ]));
  });
});
