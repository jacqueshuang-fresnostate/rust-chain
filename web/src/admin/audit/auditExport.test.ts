import { afterEach, describe, expect, it, vi } from 'vitest';

import type { AdminAuditLog } from './auditApi';
import {
  AUDIT_LOGS_EXPORT_FILENAME,
  downloadCurrentAuditLogs,
  toAuditLogsCsv
} from './auditExport';

function auditLog(): AdminAuditLog {
  return {
    action: 'asset.config.update',
    admin_id: 7,
    after_json: {
      enabled: true,
      nested: { api_key: 'new-api-secret', accessToken: 'new-token' }
    },
    before_json: {
      enabled: false,
      nested: { api_key: 'old-api-secret', accessToken: 'old-token' }
    },
    created_at: 1_735_732_800_000,
    id: 99,
    ip: '203.0.113.9',
    reason: '轮换凭据，token=reason-token',
    request_id: 'req-audit-99',
    target_id: '9',
    target_type: 'asset'
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('audit CSV export', () => {
  it('exports the current loaded rows with a UTF-8 BOM, stable filename, Chinese labels, and masked values', async () => {
    const createObjectURL = vi.fn<(blob: Blob) => string>(() => 'blob:audit-csv');
    const revokeObjectURL = vi.fn();
    Object.assign(URL, { createObjectURL, revokeObjectURL });
    let downloadedFileName = '';
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (this: HTMLAnchorElement) {
      downloadedFileName = this.download;
    });

    downloadCurrentAuditLogs([auditLog()]);

    expect(createObjectURL).toHaveBeenCalledTimes(1);
    const bytes = new Uint8Array(await createObjectURL.mock.calls[0][0].arrayBuffer());
    const csv = new TextDecoder().decode(bytes.slice(3));
    expect([...bytes.slice(0, 3)]).toEqual([0xef, 0xbb, 0xbf]);
    expect(downloadedFileName).toBe(AUDIT_LOGS_EXPORT_FILENAME);
    expect(downloadedFileName).toBe('HIPPO-审计日志-当前结果.csv');
    expect(csv).toContain('中文动作');
    expect(csv).toContain('更新资产');
    expect(csv).toContain('启用状态：否 → 是');
    expect(csv).toContain('敏感内容已遮罩');
    expect(csv).toContain('token=***');
    for (const secret of ['old-api-secret', 'new-api-secret', 'old-token', 'new-token', 'reason-token']) {
      expect(csv).not.toContain(secret);
    }
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:audit-csv');
  });

  it('quotes CSV delimiters without serializing either raw snapshot', () => {
    const log = auditLog();
    log.reason = '=HYPERLINK("https://example.invalid"),包含逗号';
    const csv = toAuditLogsCsv([log]);

    expect(csv).toContain('"\'=HYPERLINK(""https://example.invalid""),包含逗号"');
    expect(csv).not.toContain('before_json');
    expect(csv).not.toContain('after_json');
  });

  it('masks credentials in every serialized metadata and snapshot text cell', () => {
    const log = auditLog();
    log.action = 'custom.update token=action-secret';
    log.target_type = 'custom secret=type-secret';
    log.target_id = 'credential=id-secret';
    log.ip = 'token=ip-secret';
    log.request_id = 'Bearer request-secret';
    log.before_json = { diagnostic: '{"password":"snapshot-old-secret"}' };
    log.after_json = { diagnostic: 'access_token=snapshot-new-secret' };

    const csv = toAuditLogsCsv([log]);

    expect(csv).toContain('token=***');
    expect(csv).toContain('secret=***');
    expect(csv).toContain('credential=***');
    expect(csv).toContain('Bearer ***');
    for (const secret of [
      'action-secret',
      'type-secret',
      'id-secret',
      'ip-secret',
      'request-secret',
      'snapshot-old-secret',
      'snapshot-new-secret'
    ]) {
      expect(csv).not.toContain(secret);
    }
  });
});
