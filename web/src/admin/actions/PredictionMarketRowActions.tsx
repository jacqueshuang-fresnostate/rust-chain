import { Button, SideSheet, Space, Toast, Typography } from '@douyinfe/semi-ui';
import { useEffect, useMemo, useState } from 'react';

import { apiRequest } from '../../api/client';
import type { ApiRecord } from '../../api/types';
import { ConfirmAction } from '../../shared/ConfirmAction';
import type { DetailDrawerData } from '../../shared/DetailDrawer';
import { AdminMultiSelect, AdminSelect, AdminTextArea, AdminTextInput, type SemiSelectOption } from '../../shared/SemiFormControls';

const { Text } = Typography;

type PredictionMarketRowActionsProps = {
  helpers: {
    reload: () => void;
    openDetail: (detail: DetailDrawerData) => void;
  };
  record: ApiRecord;
};

type AssetConfig = {
  asset_id: number;
  asset_symbol: string;
};

type AssetConfigsResponse = {
  configs: AssetConfig[];
};

type FormValues = {
  allowedAssetIdsOverride: string[];
  displayStatus: string;
  feeRateOverride: string;
  payoutCapOverrides: string;
  settlementModeOverride: string;
};

const displayStatusOptions: SemiSelectOption[] = [
  { value: 'active', label: '显示' },
  { value: 'hidden', label: '隐藏' }
];

const settlementModeOptions: SemiSelectOption[] = [
  { value: '', label: '使用全局策略' },
  { value: 'manual_confirm', label: '外部结果 + 人工确认' },
  { value: 'auto', label: '外部结果 + 自动结算' }
];

function stringValue(value: unknown) {
  return typeof value === 'string' || typeof value === 'number' ? String(value) : '';
}

function arrayToIds(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => (typeof item === 'string' || typeof item === 'number' ? String(item) : ''))
    .filter(Boolean);
}

function recordToForm(record: ApiRecord): FormValues {
  return {
    allowedAssetIdsOverride: arrayToIds(record.allowed_asset_ids_override_json),
    displayStatus: stringValue(record.display_status) || 'active',
    feeRateOverride: stringValue(record.fee_rate_override),
    payoutCapOverrides: record.payout_cap_overrides_json ? JSON.stringify(record.payout_cap_overrides_json, null, 2) : '',
    settlementModeOverride: stringValue(record.settlement_mode_override)
  };
}

function recordId(record: ApiRecord) {
  return Number(record.id);
}

function marketCanSettle(record: ApiRecord) {
  return ['open', 'pending_confirmation'].includes(String(record.settlement_status ?? ''));
}

export function PredictionMarketRowActions({ helpers, record }: PredictionMarketRowActionsProps) {
  const [assetOptions, setAssetOptions] = useState<SemiSelectOption[]>([]);
  const [formValues, setFormValues] = useState<FormValues>(() => recordToForm(record));
  const [saving, setSaving] = useState(false);
  const [sheetVisible, setSheetVisible] = useState(false);
  const canSettle = marketCanSettle(record);
  const marketId = recordId(record);

  useEffect(() => {
    setFormValues(recordToForm(record));
  }, [record]);

  useEffect(() => {
    if (!sheetVisible) return;
    let active = true;
    apiRequest<AssetConfigsResponse>('/admin/api/v1/prediction/asset-configs')
      .then((response) => {
        if (!active) return;
        setAssetOptions(response.configs.map((asset) => ({ label: asset.asset_symbol, value: String(asset.asset_id) })));
      })
      .catch((error) => Toast.error(error instanceof Error ? error.message : '加载下注资产失败'));
    return () => {
      active = false;
    };
  }, [sheetVisible]);

  const detailData = useMemo<DetailDrawerData>(() => ({ title: '竞猜市场详情', data: record }), [record]);

  async function saveMarket() {
    setSaving(true);
    try {
      const payoutCapOverrides = formValues.payoutCapOverrides.trim() ? JSON.parse(formValues.payoutCapOverrides) : null;
      await apiRequest(`/admin/api/v1/prediction/markets/${marketId}`, {
        method: 'PATCH',
        body: JSON.stringify({
          display_status: formValues.displayStatus,
          settlement_mode_override: formValues.settlementModeOverride || null,
          allowed_asset_ids_override: formValues.allowedAssetIdsOverride.length ? formValues.allowedAssetIdsOverride.map(Number) : null,
          payout_cap_overrides: payoutCapOverrides,
          fee_rate_override: formValues.feeRateOverride.trim() ? formValues.feeRateOverride.trim() : null
        })
      });
      Toast.success('竞猜市场配置已保存');
      setSheetVisible(false);
      helpers.reload();
    } catch (error) {
      Toast.error(error instanceof Error ? error.message : '保存竞猜市场失败');
    } finally {
      setSaving(false);
    }
  }

  async function settle(result: 'yes' | 'no' | 'invalid', invalidRefundPolicy?: string) {
    await apiRequest(`/admin/api/v1/prediction/markets/${marketId}/settle`, {
      method: 'POST',
      body: JSON.stringify({
        result,
        invalid_refund_policy: invalidRefundPolicy ?? null
      })
    });
    Toast.success('竞猜市场结算已提交');
    helpers.reload();
  }

  return (
    <Space spacing={6} wrap>
      <Button onClick={() => helpers.openDetail(detailData)} theme="borderless">
        详情
      </Button>
      <Button onClick={() => setSheetVisible(true)} theme="light" type="primary">
        编辑
      </Button>
      <ConfirmAction actionText="YES" disabled={!canSettle} title="确认按 YES 结算" onConfirm={() => settle('yes')} />
      <ConfirmAction actionText="NO" disabled={!canSettle} title="确认按 NO 结算" onConfirm={() => settle('no')} />
      <ConfirmAction actionText="无效退全额" disabled={!canSettle} title="确认按无效市场退本金和手续费" onConfirm={() => settle('invalid', 'refund_stake_and_fee')} />
      <ConfirmAction actionText="无效退本金" disabled={!canSettle} title="确认按无效市场只退本金" onConfirm={() => settle('invalid', 'refund_stake_only')} />
      <SideSheet
        bodyStyle={{ overflowY: 'auto' }}
        maskClosable={false}
        onCancel={() => setSheetVisible(false)}
        title="编辑竞猜市场"
        visible={sheetVisible}
        width={560}
      >
        <Space style={{ width: '100%' }} vertical>
          <Text strong>{String(record.title ?? '-')}</Text>
          <AdminSelect
            ariaLabel="显示状态"
            onChange={(value) => setFormValues({ ...formValues, displayStatus: value })}
            optionList={displayStatusOptions}
            value={formValues.displayStatus}
          />
          <AdminSelect
            ariaLabel="结算模式覆盖"
            onChange={(value) => setFormValues({ ...formValues, settlementModeOverride: value })}
            optionList={settlementModeOptions}
            showClear
            value={formValues.settlementModeOverride}
          />
          <AdminMultiSelect
            ariaLabel="允许下注资产覆盖"
            optionList={assetOptions}
            placeholder="留空使用全局允许资产"
            value={formValues.allowedAssetIdsOverride}
            onChange={(value) => setFormValues({ ...formValues, allowedAssetIdsOverride: value })}
          />
          <AdminTextInput
            ariaLabel="手续费率覆盖"
            onChange={(value) => setFormValues({ ...formValues, feeRateOverride: value })}
            placeholder="留空使用全局手续费率"
            value={formValues.feeRateOverride}
          />
          <AdminTextArea
            ariaLabel="赔付封顶覆盖 JSON"
            autosize
            onChange={(value) => setFormValues({ ...formValues, payoutCapOverrides: value })}
            placeholder={'例如：{"1":"1000","2":"500"}；key 为资产ID'}
            value={formValues.payoutCapOverrides}
          />
          <Button loading={saving} onClick={saveMarket} theme="solid" type="primary">
            保存市场配置
          </Button>
        </Space>
      </SideSheet>
    </Space>
  );
}
