import { describe, expect, it } from 'vitest';

import {
  REDACTED_AUDIT_VALUE,
  auditActionLabel,
  auditTargetHref,
  auditTargetLabel,
  buildAuditFieldChanges,
  isSensitiveAuditKey,
  redactAuditFreeText,
  redactAuditSnapshot
} from './auditPresentation';

describe('audit presentation', () => {
  it('maps action, object type, object workspace, and Chinese field differences', () => {
    expect(auditActionLabel('asset.config.update', 'asset')).toBe('更新资产');
    expect(auditTargetLabel('asset')).toBe('资产');
    expect(auditTargetHref('asset')).toBe('/admin/assets');
    expect(auditTargetHref('prediction_asset_config')).toBe('/admin/prediction/settings?tab=assets');

    const changes = buildAuditFieldChanges(
      { enabled: false, fee_rate: '0.001', status: 'draft' },
      { enabled: true, fee_rate: '0.002', status: 'active' }
    );

    expect(changes).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: '启用状态', before: '否', after: '是' }),
      expect.objectContaining({ label: '手续费率', before: '0.001', after: '0.002' }),
      expect.objectContaining({ label: '状态', before: '草稿', after: '启用' })
    ]));
  });

  it('recursively masks token, password, secret, key, credential, and ciphertext fields without hiding real changes', () => {
    const before = {
      nested: {
        password: 'old-password',
        api_key: 'old-api-key',
        deeper: [{ accessToken: 'old-token', ciphertext: 'old-ciphertext', monkey: 'visible-old' }]
      }
    };
    const after = {
      nested: {
        password: 'new-password',
        api_key: 'new-api-key',
        deeper: [{ accessToken: 'new-token', ciphertext: 'new-ciphertext', monkey: 'visible-new' }]
      }
    };

    const changes = buildAuditFieldChanges(before, after);
    const serialized = JSON.stringify(changes);
    expect(changes.filter((change) => change.sensitive)).toHaveLength(4);
    expect(serialized).toContain(REDACTED_AUDIT_VALUE);
    expect(serialized).toContain('visible-old');
    expect(serialized).toContain('visible-new');
    for (const secret of [
      'old-password',
      'new-password',
      'old-api-key',
      'new-api-key',
      'old-token',
      'new-token',
      'old-ciphertext',
      'new-ciphertext'
    ]) {
      expect(serialized).not.toContain(secret);
    }

    expect(isSensitiveAuditKey('privateKey')).toBe(true);
    expect(isSensitiveAuditKey('monkey')).toBe(false);
    expect(redactAuditSnapshot(after)).toMatchObject({
      nested: {
        password: REDACTED_AUDIT_VALUE,
        api_key: REDACTED_AUDIT_VALUE,
        deeper: [{
          accessToken: REDACTED_AUDIT_VALUE,
          ciphertext: REDACTED_AUDIT_VALUE,
          monkey: 'visible-new'
        }]
      }
    });
  });

  it('masks credentials embedded in free-text reasons while preserving the business explanation', () => {
    const redacted = redactAuditFreeText(
      '轮换邮件凭据 token=raw-token；password: "raw password"；Bearer abc.def；JSON={"accessToken":"raw-json-token"}；保留业务说明'
    );

    expect(redacted).toContain('轮换邮件凭据');
    expect(redacted).toContain('保留业务说明');
    expect(redacted).toContain('token=***');
    expect(redacted).toContain('password: ***');
    expect(redacted).toContain('Bearer ***');
    expect(redacted).toContain('"accessToken":***');
    expect(redacted).not.toContain('raw-token');
    expect(redacted).not.toContain('raw password');
    expect(redacted).not.toContain('abc.def');
    expect(redacted).not.toContain('raw-json-token');
  });

  it('masks named credentials inside otherwise non-sensitive snapshot text values', () => {
    const before = { diagnostic: '上游返回 {"token":"old-inline-token"}', monkey: 'visible-old' };
    const after = { diagnostic: '上游返回 secret=new-inline-secret', monkey: 'visible-new' };

    const redacted = redactAuditSnapshot(after);
    const changes = buildAuditFieldChanges(before, after);
    const serialized = JSON.stringify({ redacted, changes });

    expect(serialized).toContain('上游返回');
    expect(serialized).toContain('visible-new');
    expect(serialized).toContain('secret=***');
    expect(serialized).not.toContain('old-inline-token');
    expect(serialized).not.toContain('new-inline-secret');
  });

  it('returns no field rows for identical snapshots', () => {
    expect(buildAuditFieldChanges({ enabled: true }, { enabled: true })).toEqual([]);
    expect(buildAuditFieldChanges(null, null)).toEqual([]);
  });
});
