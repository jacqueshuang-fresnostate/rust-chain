import { Button, Card, SideSheet, Space } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { AdminRequestActionBoundary } from '../../access';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { compareDecimalText, decimalFitsPrecision, isNonNegativeDecimalText } from '../../../shared/decimal';
import { AdminSelect, AdminTextInput } from '../../../shared/SemiFormControls';
import {
  type AssetOption,
  AssetSelect,
  BooleanSelect,
  type CreateActionProps,
  FormModal,
  type RowActionHelpers,
  booleanFromSelect,
  completeCreate,
  createModalProps,
  includeCurrentOption,
  openRecordDetail,
  optionalString,
  recordString,
  requiredPositiveInteger,
  requiredString,
  submitAction,
  useAssetOptions
} from './shared';

export type ConvertPairValues = {
  fromAssetId: string;
  toAssetId: string;
  pricingMode: string;
  spreadRate: string;
  feeRate: string;
  minAmount: string;
  maxAmount: string;
  targetMinAmount: string;
  targetMaxAmount: string;
  enabled: string;
};

const initialConvertPair: ConvertPairValues = {
  fromAssetId: '',
  toAssetId: '',
  pricingMode: 'fixed',
  spreadRate: '',
  feeRate: '0',
  minAmount: '',
  maxAmount: '',
  targetMinAmount: '',
  targetMaxAmount: '',
  enabled: 'true'
};

function isConvertPairCreatable(values: ConvertPairValues): boolean {
  return convertPairValidationError(values) === null;
}

export function convertPairValidationError(values: ConvertPairValues): string | null {
  try {
    const from = requiredPositiveInteger(values.fromAssetId, '源资产');
    const to = requiredPositiveInteger(values.toAssetId, '目标资产');
    if (from === to) return '源资产与目标资产必须不同';
  } catch {
    return '请选择有效的源资产和目标资产';
  }
  if (!['fixed', 'market'].includes(values.pricingMode)) return '请选择支持的定价模式';
  for (const [label, value] of [['价差率', values.spreadRate], ['手续费率', values.feeRate]]) {
    if (!isNonNegativeDecimalText(value) || compareDecimalText(value, '1') !== -1) {
      return `${label}必须大于等于 0 且小于 1（0.01 表示 1%）`;
    }
    if (!decimalFitsPrecision(value, 8)) return `${label}最多支持 8 位有效小数，不会自动舍入`;
  }
  for (const [label, minimum, maximum] of [
    ['源资产', values.minAmount, values.maxAmount],
    ['目标资产', values.targetMinAmount, values.targetMaxAmount]
  ]) {
    if (!isNonNegativeDecimalText(minimum)) return `${label}最小金额必须大于等于 0`;
    if (maximum.trim() && (!isNonNegativeDecimalText(maximum) || compareDecimalText(maximum, minimum) === -1)) {
      return `${label}最大金额必须大于等于最小金额，留空表示不限`;
    }
  }
  return null;
}

function convertPairFromRecord(record: ApiRecord): ConvertPairValues {
  return {
    fromAssetId: recordString(record, 'from_asset_id'),
    toAssetId: recordString(record, 'to_asset_id'),
    pricingMode: recordString(record, 'pricing_mode') || 'fixed',
    spreadRate: recordString(record, 'spread_rate'),
    feeRate: recordString(record, 'fee_rate') || '0',
    minAmount: recordString(record, 'min_amount'),
    maxAmount: recordString(record, 'max_amount'),
    targetMinAmount: recordString(record, 'target_min_amount'),
    targetMaxAmount: recordString(record, 'target_max_amount'),
    enabled: record.enabled === false ? 'false' : 'true'
  };
}

function convertPairRequestBody(values: ConvertPairValues, reason: string) {
  const error = convertPairValidationError(values);
  if (error) throw new Error(error);
  return {
    from_asset_id: requiredPositiveInteger(values.fromAssetId, '源资产'),
    to_asset_id: requiredPositiveInteger(values.toAssetId, '目标资产'),
    pricing_mode: requiredString(values.pricingMode, '定价模式'),
    spread_rate: requiredString(values.spreadRate, '价差率'),
    fee_rate: requiredString(values.feeRate, '手续费率'),
    min_amount: requiredString(values.minAmount, '源资产最小金额'),
    max_amount: optionalString(values.maxAmount),
    target_min_amount: requiredString(values.targetMinAmount, '目标资产最小金额'),
    target_max_amount: optionalString(values.targetMaxAmount),
    enabled: booleanFromSelect(values.enabled),
    reason
  };
}

function convertPairUpdateRequestBody(values: ConvertPairValues, reason: string) {
  return {
    ...convertPairRequestBody(values, reason),
    max_amount: optionalString(values.maxAmount) ?? null,
    target_max_amount: optionalString(values.targetMaxAmount) ?? null
  };
}

function ConvertPairEditAction({ helpers, pairId, record }: { helpers: RowActionHelpers; pairId: string; record: ApiRecord }) {
  const [config, setConfig] = useState(() => convertPairFromRecord(record));
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const assetOptionsWithCurrent = includeCurrentOption(
    includeCurrentOption(assetOptions, config.fromAssetId, `${recordString(record, 'from_asset_symbol') || `资产${config.fromAssetId}`}（ID: ${config.fromAssetId}）`),
    config.toAssetId,
    `${recordString(record, 'to_asset_symbol') || `资产${config.toAssetId}`}（ID: ${config.toAssetId}）`
  );

  return (
    <>
      <Button disabled={!pairId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改闪兑交易对" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <ConvertPairFields assetLoading={assetLoading} assetOptions={assetOptionsWithCurrent} values={config} onChange={setConfig} />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isConvertPairCreatable(config)}
              title="确认修改闪兑交易对"
              onConfirm={async (reason) => {
                await submitAction('修改闪兑交易对', () =>
                  apiRequest(`/admin/api/v1/convert/pairs/${pairId}`, {
                    method: 'PATCH',
                    body: JSON.stringify(convertPairUpdateRequestBody(config, reason))
                  })
                );
                setVisible(false);
                helpers.reload();
              }}
            />
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

export function ConvertPairRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const pairId = recordString(record, 'id');
  const enabled = record.enabled === true;
  const nextEnabled = !enabled;
  const actionText = enabled ? '禁用' : '启用';

  return (
    <>
      <Button disabled={!pairId} onClick={() => openRecordDetail('/admin/api/v1/convert/pairs', pairId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <AdminRequestActionBoundary endpoint={`/admin/api/v1/convert/pairs/${pairId}`} method="PATCH">
        <ConvertPairEditAction helpers={helpers} pairId={pairId} record={record} />
        <ConfirmAction
          actionText={actionText}
          disabled={!pairId}
          title={`${actionText}闪兑交易对`}
          onConfirm={async (reason) => {
            await submitAction(`${actionText}闪兑交易对`, () =>
              apiRequest(`/admin/api/v1/convert/pairs/${pairId}`, {
                method: 'PATCH',
                body: JSON.stringify({ enabled: nextEnabled, reason })
              })
            );
            helpers.reload();
          }}
        />
      </AdminRequestActionBoundary>
      {!enabled ? (
        <AdminRequestActionBoundary endpoint={`/admin/api/v1/convert/pairs/${pairId}`} method="DELETE">
        <ConfirmAction
          actionText="删除"
          disabled={!pairId}
          title="确认删除闪兑交易对"
          onConfirm={async (reason) => {
            await submitAction('删除闪兑交易对', () =>
              apiRequest(`/admin/api/v1/convert/pairs/${pairId}`, {
                method: 'DELETE',
                body: JSON.stringify({ reason })
              })
            );
            helpers.reload();
          }}
        />
        </AdminRequestActionBoundary>
      ) : null}
    </>
  );
}

export function ConvertOrderRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const orderId = recordString(record, 'id');

  return (
    <Button disabled={!orderId} onClick={() => openRecordDetail('/admin/api/v1/convert/orders', orderId, helpers)} size="small" theme="borderless">
      查看详情
    </Button>
  );
}

function ConvertPairFields({
  assetLoading,
  assetOptions,
  onChange,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  onChange: (values: ConvertPairValues) => void;
  values: ConvertPairValues;
}) {
  const patch = (nextValues: Partial<ConvertPairValues>) => onChange({ ...values, ...nextValues });

  return (
    <div className="admin-action-form">
      <AssetSelect label="源资产" loading={assetLoading} options={assetOptions} value={values.fromAssetId} onChange={(fromAssetId) => patch({ fromAssetId })} />
      <AssetSelect label="目标资产" loading={assetLoading} options={assetOptions} value={values.toAssetId} onChange={(toAssetId) => patch({ toAssetId })} />
      <label>
        定价模式
        <AdminSelect
          ariaLabel="定价模式"
          onChange={(pricingMode) => patch({ pricingMode })}
          optionList={[
            { value: 'fixed', label: '固定价格' },
            { value: 'market', label: '市场价格' }
          ]}
          value={values.pricingMode}
        />
      </label>
      <label>价差率<AdminTextInput ariaLabel="价差率" value={values.spreadRate} onChange={(spreadRate) => patch({ spreadRate })} /></label>
      <label>手续费率<AdminTextInput ariaLabel="手续费率" value={values.feeRate} onChange={(feeRate) => patch({ feeRate })} /></label>
      <span className="admin-form-hint">价差率和手续费率按比例填写：0.01 表示 1%；取值大于等于 0、小于 1，最多支持 8 位有效小数。</span>
      <label>源资产最小金额<AdminTextInput ariaLabel="源资产最小金额" value={values.minAmount} onChange={(minAmount) => patch({ minAmount })} /></label>
      <label>源资产最大金额<AdminTextInput ariaLabel="源资产最大金额" value={values.maxAmount} onChange={(maxAmount) => patch({ maxAmount })} /></label>
      <label>目标资产最小金额<AdminTextInput ariaLabel="目标资产最小金额" value={values.targetMinAmount} onChange={(targetMinAmount) => patch({ targetMinAmount })} /></label>
      <label>目标资产最大金额<AdminTextInput ariaLabel="目标资产最大金额" value={values.targetMaxAmount} onChange={(targetMaxAmount) => patch({ targetMaxAmount })} /></label>
      <label>启用<BooleanSelect label="启用" value={values.enabled} onChange={(enabled) => patch({ enabled })} /></label>
      {values.fromAssetId && values.toAssetId && convertPairValidationError(values) ? (
        <span className="admin-reference-field__error" role="alert">{convertPairValidationError(values)}</span>
      ) : null}
    </div>
  );
}

export function CreateConvertPairAction({ onCreated }: CreateActionProps = {}) {
  const [convertPair, setConvertPair] = useState(initialConvertPair);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <FormModal actionText="添加闪兑交易对" size="wide" title="添加闪兑交易对">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <ConvertPairFields assetLoading={assetLoading} assetOptions={assetOptions} values={convertPair} onChange={setConvertPair} />
          <ConfirmAction
            actionText="提交添加闪兑交易对"
            disabled={!isConvertPairCreatable(convertPair)}
            title="确认添加闪兑交易对"
            onConfirm={async (reason) => {
              await submitAction('添加闪兑交易对', async () => {
                await apiRequest('/admin/api/v1/convert/pairs', {
                  method: 'POST',
                  body: JSON.stringify(convertPairRequestBody(convertPair, reason))
                });
              });
              completeCreate(close, onCreated, () => setConvertPair(initialConvertPair));
            }}
          />
        </Space>
      </Card>
      )}
    </FormModal>
  );
}
