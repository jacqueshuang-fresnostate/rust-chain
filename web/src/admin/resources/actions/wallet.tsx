import { IconUpload } from '@douyinfe/semi-icons';
import { Button, Card, Col, Modal, Popconfirm, Row, SideSheet, Space, Toast, Typography, Upload } from '@douyinfe/semi-ui';
import type { customRequestArgs } from '@douyinfe/semi-ui/lib/es/upload';
import { useEffect, useState } from 'react';

import { listAdminResource } from '../../../api/adminResources';
import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminImageUpload } from '../../../shared/AdminImageUpload';
import { AdminMultiSelect, AdminSelect, AdminSwitch, AdminTextArea, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import {
  type AssetOption,
  AssetStatusSelect,
  type CreateActionProps,
  FormModal,
  type RowActionHelpers,
  completeCreate,
  createModalProps,
  errorMessage,
  isNonNegativeDecimalInput,
  isNonNegativeIntegerInput,
  openRecordDetail,
  optionalString,
  recordString,
  requiredNonNegativeDecimal,
  requiredNonNegativeInteger,
  requiredString,
  statusOptions,
  submitAction,
  useAssetOptions
} from './shared';

const { Text } = Typography;

type AssetValues = {
  logoUrl: string;
  symbol: string;
  name: string;
  precisionScale: string;
  assetType: string;
  status: string;
  depositEnabled: boolean;
  withdrawEnabled: boolean;
  minDepositAmount: string;
  depositFee: string;
  withdrawFee: string;
  withdrawFeeTiers: AssetWithdrawFeeTierValues[];
};

type AssetConfigValues = {
  logoUrl: string;
  name: string;
  precisionScale: string;
  assetType: string;
  status: string;
  depositEnabled: boolean;
  withdrawEnabled: boolean;
  minDepositAmount: string;
  depositFee: string;
  withdrawFee: string;
  withdrawFeeTiers: AssetWithdrawFeeTierValues[];
};

type AssetWithdrawFeeTierValues = {
  minAmount: string;
  maxAmount: string;
  feeRatePercent: string;
};

type DepositAddressPoolValues = {
  network: string;
  addressGroupCode: string;
  address: string;
  assetSymbols: string[];
  status: string;
  memo: string;
  remark: string;
};

type DepositAddressPoolEntryValues = {
  address: string;
  memo: string;
  remark: string;
};

type DepositAddressPoolCreateValues = {
  addressGroupCode: string;
  assetSymbols: string[];
  entries: DepositAddressPoolEntryValues[];
  network: string;
  status: string;
};

type DepositNetworkConfigValues = {
  network: string;
  displayName: string;
  addressGroupCode: string;
  addressGroupName: string;
  assetSymbols: string[];
  status: string;
  sortOrder: string;
};

type DepositNetworkConfigOption = {
  addressGroupCode: string;
  addressGroupName: string;
  assetSymbols: string[];
  displayName: string;
  label: string;
  network: string;
};

const initialDepositAddressPoolEntry: DepositAddressPoolEntryValues = {
  address: '',
  memo: '',
  remark: ''
};

function createInitialDepositAddressPoolCreate(): DepositAddressPoolCreateValues {
  return {
    addressGroupCode: 'A',
    assetSymbols: [],
    entries: [{ ...initialDepositAddressPoolEntry }],
    network: 'eth',
    status: 'available'
  };
}

const initialDepositNetworkConfig: DepositNetworkConfigValues = {
  network: 'eth',
  displayName: 'Ethereum',
  addressGroupCode: 'A',
  addressGroupName: 'EVM',
  assetSymbols: [],
  status: 'active',
  sortOrder: '0'
};

const initialAsset: AssetValues = {
  logoUrl: '',
  symbol: '',
  name: '',
  precisionScale: '8',
  assetType: 'coin',
  status: 'active',
  depositEnabled: true,
  withdrawEnabled: true,
  minDepositAmount: '0',
  depositFee: '0',
  withdrawFee: '0',
  withdrawFeeTiers: []
};

function isWithdrawFeeTiersInputValid(tiers: AssetWithdrawFeeTierValues[]): boolean {
  return tiers.every((tier) => {
    const minAmount = tier.minAmount.trim();
    const maxAmount = tier.maxAmount.trim();
    return (
      isNonNegativeDecimalInput(minAmount) &&
      (!maxAmount || (isNonNegativeDecimalInput(maxAmount) && Number(maxAmount) > Number(minAmount))) &&
      isNonNegativeDecimalInput(tier.feeRatePercent)
    );
  });
}

function recordWithdrawFeeTiers(record: ApiRecord): AssetWithdrawFeeTierValues[] {
  const rawValue = record.withdraw_fee_tiers;
  const rawTiers = Array.isArray(rawValue) ? rawValue : parseJsonArray(rawValue);
  return rawTiers
    .map((item) => {
      if (!item || typeof item !== 'object') return null;
      const tier = item as Record<string, unknown>;
      return {
        minAmount: formValueString(tier.min_amount),
        maxAmount: formValueString(tier.max_amount),
        feeRatePercent: formValueString(tier.fee_rate_percent)
      };
    })
    .filter((tier): tier is AssetWithdrawFeeTierValues => Boolean(tier && tier.minAmount && tier.feeRatePercent));
}

function withdrawFeeTierPayload(tiers: AssetWithdrawFeeTierValues[]) {
  return tiers.map((tier, index) => {
    const minAmount = requiredNonNegativeDecimal(tier.minAmount, `第${index + 1}档最小金额`);
    const maxAmount = tier.maxAmount.trim() ? requiredNonNegativeDecimal(tier.maxAmount, `第${index + 1}档最大金额`) : null;
    if (maxAmount !== null && Number(maxAmount) <= Number(minAmount)) {
      throw new Error(`第${index + 1}档最大金额必须大于最小金额`);
    }
    return {
      min_amount: minAmount,
      max_amount: maxAmount,
      fee_rate_percent: requiredNonNegativeDecimal(tier.feeRatePercent, `第${index + 1}档手续费比例`)
    };
  });
}

function parseJsonArray(value: unknown): unknown[] {
  if (typeof value !== 'string' || !value.trim()) return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function formValueString(value: unknown): string {
  return typeof value === 'number' || typeof value === 'string' ? String(value) : '';
}

function isDepositAddressPoolSubmittable(values: DepositAddressPoolValues): boolean {
  return Boolean(values.network.trim() && values.address.trim() && values.status.trim());
}

function isDepositAddressPoolCreateSubmittable(values: DepositAddressPoolCreateValues): boolean {
  return Boolean(values.network.trim() && values.addressGroupCode.trim() && values.status.trim() && values.entries.length > 0 && values.entries.every((entry) => entry.address.trim()));
}

function isDepositNetworkConfigSubmittable(values: DepositNetworkConfigValues): boolean {
  return Boolean(values.network.trim() && values.displayName.trim() && values.addressGroupCode.trim() && values.status.trim() && isNonNegativeIntegerInput(values.sortOrder));
}

function depositAddressPoolFromRecord(record: ApiRecord): DepositAddressPoolValues {
  const assetSymbols = Array.isArray(record.asset_symbols) ? record.asset_symbols.filter((value): value is string => typeof value === 'string') : [];
  const network = recordString(record, 'network') || 'eth';
  return {
    network,
    addressGroupCode: recordString(record, 'address_group_code') || defaultDepositAddressGroupCode(network),
    address: recordString(record, 'address'),
    assetSymbols: assetSymbols.length > 0 ? assetSymbols : recordString(record, 'asset_symbol') ? [recordString(record, 'asset_symbol')] : [],
    status: recordString(record, 'status') === 'disabled' ? 'disabled' : 'available',
    memo: recordString(record, 'memo'),
    remark: recordString(record, 'remark')
  };
}

function normalizedDepositAssetSymbols(values: string[]): string[] {
  return values.map((value) => value.trim().toUpperCase()).filter((value, index, items) => value && items.indexOf(value) === index);
}

function depositAddressPoolRequestBody(values: DepositAddressPoolValues, reason: string) {
  const assetSymbols = normalizedDepositAssetSymbols(values.assetSymbols);
  return {
    network: requiredString(values.network, '网络'),
    address_group_code: requiredString(values.addressGroupCode, '地址集合编号'),
    address: requiredString(values.address, '充值地址'),
    asset_symbols: assetSymbols,
    status: requiredString(values.status, '状态'),
    memo: optionalString(values.memo),
    remark: optionalString(values.remark),
    reason
  };
}

function depositAddressPoolBatchRequestBody(values: DepositAddressPoolCreateValues, reason: string) {
  return {
    network: requiredString(values.network, '网络'),
    address_group_code: requiredString(values.addressGroupCode, '地址集合编号'),
    asset_symbols: normalizedDepositAssetSymbols(values.assetSymbols),
    status: requiredString(values.status, '状态'),
    entries: values.entries.map((entry) => ({
      address: requiredString(entry.address, '充值地址'),
      memo: optionalString(entry.memo),
      remark: optionalString(entry.remark)
    })),
    reason
  };
}

function depositNetworkConfigFromRecord(record: ApiRecord): DepositNetworkConfigValues {
  const assetSymbols = Array.isArray(record.asset_symbols) ? record.asset_symbols.filter((value): value is string => typeof value === 'string') : [];
  return {
    network: recordString(record, 'network') || 'eth',
    displayName: recordString(record, 'display_name'),
    addressGroupCode: recordString(record, 'address_group_code'),
    addressGroupName: recordString(record, 'address_group_name'),
    assetSymbols,
    status: recordString(record, 'status') === 'disabled' ? 'disabled' : 'active',
    sortOrder: recordString(record, 'sort_order') || '0'
  };
}

function depositNetworkConfigRequestBody(values: DepositNetworkConfigValues, reason: string) {
  return {
    network: requiredString(values.network, '网络'),
    display_name: requiredString(values.displayName, '显示名称'),
    address_group_code: requiredString(values.addressGroupCode, '地址集合编号'),
    address_group_name: optionalString(values.addressGroupName),
    asset_symbols: normalizedDepositAssetSymbols(values.assetSymbols),
    status: requiredString(values.status, '状态'),
    sort_order: Number.parseInt(values.sortOrder, 10),
    reason
  };
}

function depositAddressImportDelimiter(line: string): string {
  if (line.includes('\t')) {
    return '\t';
  }
  if (line.includes('|')) {
    return '|';
  }
  return ',';
}

function parseDelimitedImportLine(line: string, delimiter: string): string[] {
  const cells: string[] = [];
  let current = '';
  let quoted = false;

  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    if (char === '"') {
      if (quoted && line[index + 1] === '"') {
        current += '"';
        index += 1;
      } else {
        quoted = !quoted;
      }
      continue;
    }
    if (char === delimiter && !quoted) {
      cells.push(current.trim());
      current = '';
      continue;
    }
    current += char;
  }

  cells.push(current.trim());
  return cells;
}

function isDepositAddressImportHeader(cells: string[]): boolean {
  const firstCell = (cells[0] ?? '').replace(/^\uFEFF/, '').trim().toLowerCase();
  return ['address', 'deposit address', '充值地址', '地址'].includes(firstCell);
}

function parseDepositAddressImportText(content: string): DepositAddressPoolEntryValues[] {
  const entries: DepositAddressPoolEntryValues[] = [];
  const lines = content.replace(/\r\n/g, '\n').replace(/\r/g, '\n').split('\n');

  lines.forEach((line, index) => {
    if (!line.trim()) {
      return;
    }

    const cells = parseDelimitedImportLine(line, depositAddressImportDelimiter(line));
    const address = (cells[0] ?? '').replace(/^\uFEFF/, '').trim();
    if (index === 0 && isDepositAddressImportHeader(cells)) {
      return;
    }
    if (!address) {
      throw new Error(`第 ${index + 1} 行缺少充值地址`);
    }

    entries.push({
      address,
      memo: (cells[1] ?? '').trim(),
      remark: cells.slice(2).join('，').trim()
    });
  });

  if (!entries.length) {
    throw new Error('导入文件没有可用的充值地址');
  }

  return entries;
}

function hasDepositAddressEntryContent(entry: DepositAddressPoolEntryValues): boolean {
  return Boolean(entry.address.trim() || entry.memo.trim() || entry.remark.trim());
}

function readDepositAddressImportFile(file: File): Promise<string> {
  if (typeof file.text === 'function') {
    return file.text();
  }

  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error('读取导入文件失败'));
    reader.onload = () => resolve(String(reader.result ?? ''));
    reader.readAsText(file);
  });
}

function isAssetCreatable(values: AssetValues): boolean {
  return Boolean(
    values.symbol.trim() &&
      values.name.trim() &&
      isNonNegativeIntegerInput(values.precisionScale) &&
      isNonNegativeDecimalInput(values.minDepositAmount) &&
      isNonNegativeDecimalInput(values.depositFee) &&
      isNonNegativeDecimalInput(values.withdrawFee) &&
      isWithdrawFeeTiersInputValid(values.withdrawFeeTiers)
  );
}

function isAssetConfigUpdatable(values: AssetConfigValues): boolean {
  return Boolean(
    values.name.trim() &&
      isNonNegativeIntegerInput(values.precisionScale) &&
      values.assetType.trim() &&
      values.status.trim() &&
      isNonNegativeDecimalInput(values.minDepositAmount) &&
      isNonNegativeDecimalInput(values.depositFee) &&
      isNonNegativeDecimalInput(values.withdrawFee) &&
      isWithdrawFeeTiersInputValid(values.withdrawFeeTiers)
  );
}

function toDepositNetworkConfigOption(record: ApiRecord): DepositNetworkConfigOption | null {
  const network = recordString(record, 'network');
  const addressGroupCode = recordString(record, 'address_group_code');
  if (!network || !addressGroupCode) {
    return null;
  }
  const displayName = recordString(record, 'display_name') || network;
  const addressGroupName = recordString(record, 'address_group_name');
  const assetSymbols = Array.isArray(record.asset_symbols)
    ? record.asset_symbols.filter((value): value is string => typeof value === 'string')
    : [];
  return {
    addressGroupCode,
    addressGroupName,
    assetSymbols,
    displayName,
    label: addressGroupName ? `${displayName} · ${addressGroupCode}（${addressGroupName}）` : `${displayName} · ${addressGroupCode}`,
    network
  };
}

function useDepositNetworkConfigOptions(enabled = true) {
  const [networkConfigs, setNetworkConfigs] = useState<DepositNetworkConfigOption[]>([]);
  const [networkConfigLoading, setNetworkConfigLoading] = useState(false);

  useEffect(() => {
    if (!enabled) {
      return undefined;
    }

    let active = true;
    setNetworkConfigLoading(true);

    listAdminResource('/admin/api/v1/deposit-network-configs', 'configs', { status: 'active', limit: 100 })
      .then((result) => {
        if (!active) {
          return;
        }

        setNetworkConfigs(result.rows.map(toDepositNetworkConfigOption).filter((config): config is DepositNetworkConfigOption => config !== null));
      })
      .catch(() => {
        if (active) {
          setNetworkConfigs([]);
        }
      })
      .finally(() => {
        if (active) {
          setNetworkConfigLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [enabled]);

  return { networkConfigLoading, networkConfigs };
}

function AssetSymbolMultiSelect({
  label,
  loading,
  onChange,
  options,
  value
}: {
  label: string;
  loading: boolean;
  onChange: (value: string[]) => void;
  options: AssetOption[];
  value: string[];
}) {
  const symbolOptions = options
    .filter((asset) => asset.symbol)
    .map((asset) => ({ value: asset.symbol, label: asset.label }));

  return (
    <label>
      {label}
      <AdminMultiSelect
        ariaLabel={label}
        disabled={loading}
        loading={loading}
        onChange={onChange}
        optionList={symbolOptions}
        placeholder={loading ? '加载资产中...' : '留空表示该网络任意资产'}
        value={value}
      />
    </label>
  );
}

const assetTypeOptions: SemiSelectOption[] = [
  { value: 'coin', label: '数字货币' },
  { value: 'stablecoin', label: '稳定币' },
  { value: 'fiat', label: '法币' },
  { value: 'platform', label: '平台币' }
];

const depositNetworkOptions: SemiSelectOption[] = [
  { value: 'eth', label: 'ETH' },
  { value: 'base', label: 'Base' },
  { value: 'tron', label: 'Tron' },
  { value: 'btc', label: 'BTC' },
  { value: 'solana', label: 'Solana' }
];

function depositNetworkSelectOptions(configs: DepositNetworkConfigOption[]): SemiSelectOption[] {
  return configs.length > 0 ? configs.map((config) => ({ value: config.network, label: config.label })) : depositNetworkOptions;
}

function depositNetworkConfigForNetwork(configs: DepositNetworkConfigOption[], network: string): DepositNetworkConfigOption | undefined {
  return configs.find((config) => config.network === network);
}

function defaultDepositAddressGroupCode(network: string): string {
  switch (network) {
    case 'eth':
    case 'base':
      return 'A';
    case 'btc':
      return 'B';
    case 'tron':
      return 'C';
    case 'solana':
      return 'D';
    default:
      return network.toUpperCase();
  }
}

function depositAddressGroupForNetwork(configs: DepositNetworkConfigOption[], network: string, fallback = ''): string {
  return depositNetworkConfigForNetwork(configs, network)?.addressGroupCode || fallback || defaultDepositAddressGroupCode(network);
}

function depositAssetOptionsForNetwork(options: AssetOption[], config?: DepositNetworkConfigOption): AssetOption[] {
  if (!config || config.assetSymbols.length === 0) {
    return options;
  }
  const allowed = new Set(config.assetSymbols.map((symbol) => symbol.toUpperCase()));
  return options.filter((asset) => allowed.has(asset.symbol.toUpperCase()));
}

function normalizeSelectedDepositAssetsForNetwork(values: string[], config?: DepositNetworkConfigOption): string[] {
  if (!config || config.assetSymbols.length === 0) {
    return values;
  }
  const allowed = new Set(config.assetSymbols.map((symbol) => symbol.toUpperCase()));
  return values.filter((symbol) => allowed.has(symbol.toUpperCase()));
}

const depositAddressStatusOptions: SemiSelectOption[] = [
  { value: 'available', label: '可用' },
  { value: 'disabled', label: '禁用' }
];

function AssetTypeSelect({ onChange, value }: { onChange: (value: string) => void; value: string }) {
  return <AdminSelect ariaLabel="资产类型" onChange={onChange} optionList={assetTypeOptions} value={value} />;
}

function AssetWithdrawFeeTiersEditor({
  onChange,
  values
}: {
  onChange: (values: AssetWithdrawFeeTierValues[]) => void;
  values: AssetWithdrawFeeTierValues[];
}) {
  const updateTier = (index: number, patch: Partial<AssetWithdrawFeeTierValues>) => {
    onChange(values.map((tier, tierIndex) => (tierIndex === index ? { ...tier, ...patch } : tier)));
  };
  const removeTier = (index: number) => {
    onChange(values.filter((_, tierIndex) => tierIndex !== index));
  };

  return (
    <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
      <Space align="center" style={{ justifyContent: 'space-between', width: '100%' }}>
        <Text strong>提现手续费梯度</Text>
        <Button
          onClick={() => onChange([...values, { minAmount: '', maxAmount: '', feeRatePercent: '' }])}
          size="small"
          theme="light"
          type="primary"
        >
          添加梯度
        </Button>
      </Space>
      {values.length === 0 ? <Text type="tertiary">未配置时使用固定提现手续费。</Text> : null}
      {values.map((tier, index) => (
        <Row gutter={12} key={index} style={{ width: '100%' }}>
          <Col span={7}>
            <label>
              第{index + 1}档最小金额
              <AdminTextInput ariaLabel={`梯度最小金额 ${index + 1}`} value={tier.minAmount} onChange={(minAmount) => updateTier(index, { minAmount })} />
            </label>
          </Col>
          <Col span={7}>
            <label>
              最大金额
              <AdminTextInput ariaLabel={`梯度最大金额 ${index + 1}`} placeholder="留空为无上限" value={tier.maxAmount} onChange={(maxAmount) => updateTier(index, { maxAmount })} />
            </label>
          </Col>
          <Col span={6}>
            <label>
              手续费比例%
              <AdminTextInput ariaLabel={`梯度手续费比例 ${index + 1}`} value={tier.feeRatePercent} onChange={(feeRatePercent) => updateTier(index, { feeRatePercent })} />
            </label>
          </Col>
          <Col span={4}>
            <Button onClick={() => removeTier(index)} size="small" theme="borderless" type="danger">
              删除
            </Button>
          </Col>
        </Row>
      ))}
    </Space>
  );
}

function AssetEditAction({ assetId, helpers, record }: { assetId: string; helpers: RowActionHelpers; record: ApiRecord }) {
  const [config, setConfig] = useState<AssetConfigValues>({
    logoUrl: recordString(record, 'logo_url'),
    name: recordString(record, 'name'),
    precisionScale: recordString(record, 'precision_scale'),
    assetType: recordString(record, 'asset_type') || 'coin',
    status: recordString(record, 'status') || 'active',
    depositEnabled: record.deposit_enabled !== false,
    withdrawEnabled: record.withdraw_enabled !== false,
    minDepositAmount: recordString(record, 'min_deposit_amount') || '0',
    depositFee: recordString(record, 'deposit_fee') || '0',
    withdrawFee: recordString(record, 'withdraw_fee') || '0',
    withdrawFeeTiers: recordWithdrawFeeTiers(record)
  });
  const [visible, setVisible] = useState(false);

  return (
    <>
      <Button disabled={!assetId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改资产配置" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <div className="admin-action-form">
              <label>资产符号<AdminTextInput ariaLabel="资产符号" readOnly value={recordString(record, 'symbol')} onChange={() => undefined} /></label>
              <label>资产名称<AdminTextInput ariaLabel="资产名称" value={config.name} onChange={(name) => setConfig({ ...config, name })} /></label>
              <AdminImageUpload label="资产 Logo" value={config.logoUrl} variant="avatar" onChange={(logoUrl) => setConfig({ ...config, logoUrl })} />
              <label>资产精度<AdminTextInput ariaLabel="资产精度" value={config.precisionScale} onChange={(precisionScale) => setConfig({ ...config, precisionScale })} /></label>
              <label>资产类型<AssetTypeSelect value={config.assetType} onChange={(assetType) => setConfig({ ...config, assetType })} /></label>
              <label>状态<AssetStatusSelect value={config.status} onChange={(status) => setConfig({ ...config, status })} /></label>
              <AdminSwitch checked={config.depositEnabled} label="支持充值" onChange={(depositEnabled) => setConfig({ ...config, depositEnabled })} />
              <AdminSwitch checked={config.withdrawEnabled} label="支持提现" onChange={(withdrawEnabled) => setConfig({ ...config, withdrawEnabled })} />
              <label>最小充值数量<AdminTextInput ariaLabel="最小充值数量" value={config.minDepositAmount} onChange={(minDepositAmount) => setConfig({ ...config, minDepositAmount })} /></label>
              <label>充值手续费<AdminTextInput ariaLabel="充值手续费" value={config.depositFee} onChange={(depositFee) => setConfig({ ...config, depositFee })} /></label>
              <label>提现手续费<AdminTextInput ariaLabel="提现手续费" value={config.withdrawFee} onChange={(withdrawFee) => setConfig({ ...config, withdrawFee })} /></label>
              <AssetWithdrawFeeTiersEditor values={config.withdrawFeeTiers} onChange={(withdrawFeeTiers) => setConfig({ ...config, withdrawFeeTiers })} />
            </div>
            <ConfirmAction
              actionText="提交修改"
              disabled={!isAssetConfigUpdatable(config)}
              title="确认修改资产配置"
              onConfirm={async (reason) => {
                await submitAction('修改资产配置', () =>
                  apiRequest(`/admin/api/v1/assets/${assetId}`, {
                    method: 'PATCH',
                    body: JSON.stringify({
                      name: requiredString(config.name, '资产名称'),
                      logo_url: optionalString(config.logoUrl),
                      precision_scale: requiredNonNegativeInteger(config.precisionScale, '资产精度'),
                      asset_type: requiredString(config.assetType, '资产类型'),
                      status: requiredString(config.status, '状态'),
                      deposit_enabled: config.depositEnabled,
                      withdraw_enabled: config.withdrawEnabled,
                      min_deposit_amount: requiredNonNegativeDecimal(config.minDepositAmount, '最小充值数量'),
                      deposit_fee: requiredNonNegativeDecimal(config.depositFee, '充值手续费'),
                      withdraw_fee: requiredNonNegativeDecimal(config.withdrawFee, '提现手续费'),
                      withdraw_fee_tiers: withdrawFeeTierPayload(config.withdrawFeeTiers),
                      reason
                    })
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

export function AssetRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const assetId = recordString(record, 'id');
  const status = recordString(record, 'status');

  return (
    <>
      <Button disabled={!assetId} onClick={() => openRecordDetail('/admin/api/v1/assets', assetId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <AssetEditAction assetId={assetId} helpers={helpers} record={record} />
      {status === 'disabled' ? (
        <ConfirmAction
          actionText="删除"
          disabled={!assetId}
          title="确认删除资产"
          onConfirm={async (reason) => {
            await submitAction('删除资产', () =>
              apiRequest(`/admin/api/v1/assets/${assetId}`, {
                method: 'DELETE',
                body: JSON.stringify({ reason })
              })
            );
            helpers.reload();
          }}
        />
      ) : null}
    </>
  );
}

function DepositAddressPoolCreateFields({
  assetLoading,
  assetOptions,
  networkConfigLoading,
  networkConfigs,
  onChange,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  networkConfigLoading: boolean;
  networkConfigs: DepositNetworkConfigOption[];
  onChange: (values: DepositAddressPoolCreateValues) => void;
  values: DepositAddressPoolCreateValues;
}) {
  const currentNetworkConfig = depositNetworkConfigForNetwork(networkConfigs, values.network);
  const filteredAssetOptions = depositAssetOptionsForNetwork(assetOptions, currentNetworkConfig);
  const networkOptions = depositNetworkSelectOptions(networkConfigs);
  const updateNetwork = (network: string) => {
    const nextConfig = depositNetworkConfigForNetwork(networkConfigs, network);
    onChange({
      ...values,
      network,
      addressGroupCode: nextConfig?.addressGroupCode || defaultDepositAddressGroupCode(network),
      assetSymbols: normalizeSelectedDepositAssetsForNetwork(values.assetSymbols, nextConfig)
    });
  };
  const updateEntry = (index: number, nextEntry: DepositAddressPoolEntryValues) => {
    onChange({
      ...values,
      entries: values.entries.map((entry, entryIndex) => (entryIndex === index ? nextEntry : entry))
    });
  };
  const removeEntry = (index: number) => {
    if (values.entries.length <= 1) {
      return;
    }
    onChange({ ...values, entries: values.entries.filter((_, entryIndex) => entryIndex !== index) });
  };
  const appendEntry = () => {
    onChange({ ...values, entries: [...values.entries, { ...initialDepositAddressPoolEntry }] });
  };
  const importAddressFile = async (request: customRequestArgs) => {
    try {
      const importedEntries = parseDepositAddressImportText(await readDepositAddressImportFile(request.fileInstance));
      const existingEntries = values.entries.filter(hasDepositAddressEntryContent);
      onChange({
        ...values,
        entries: existingEntries.length ? [...existingEntries, ...importedEntries] : importedEntries
      });
      Toast.success(`已导入 ${importedEntries.length} 条充值地址`);
      request.onSuccess({ imported: importedEntries.length });
    } catch (error) {
      Toast.error(errorMessage(error));
      request.onError({ status: 400 });
    }
  };

  return (
    <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
      <section aria-label="充值地址规则" style={{ width: '100%' }}>
        <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
          <strong>地址规则</strong>
          <Row gutter={[16, 16]} style={{ width: '100%' }}>
            <Col xs={24} md={6}>
              <div className="admin-action-form">
                <label>
                  网络
                  <AdminSelect ariaLabel="网络" loading={networkConfigLoading} onChange={updateNetwork} optionList={networkOptions} value={values.network} />
                </label>
              </div>
            </Col>
            <Col xs={24} md={5}>
              <div className="admin-action-form">
                <label>
                  地址集合编号
                  <AdminTextInput ariaLabel="地址集合编号" readOnly value={values.addressGroupCode} onChange={() => undefined} />
                </label>
              </div>
            </Col>
            <Col xs={24} md={8}>
              <div className="admin-action-form">
                <AssetSymbolMultiSelect
                  label="支持币种"
                  loading={assetLoading}
                  options={filteredAssetOptions}
                  value={values.assetSymbols}
                  onChange={(assetSymbols) => onChange({ ...values, assetSymbols })}
                />
              </div>
            </Col>
            <Col xs={24} md={5}>
              <div className="admin-action-form">
                <label>
                  初始状态
                  <AdminSelect ariaLabel="初始状态" onChange={(status) => onChange({ ...values, status })} optionList={depositAddressStatusOptions} value={values.status} />
                </label>
              </div>
            </Col>
            <Col xs={24} md={5}>
              <div className="admin-action-form">
                <label>
                  导入文件
                  <Upload
                    accept=".csv,.txt"
                    action="/admin/local/deposit-address-import"
                    customRequest={importAddressFile}
                    limit={1}
                    onAcceptInvalid={() => Toast.error('请导入 CSV 或 TXT 文件')}
                    showUploadList={false}
                  >
                    <Button icon={<IconUpload aria-hidden="true" />} theme="borderless" type="primary">
                      导入地址
                    </Button>
                  </Upload>
                </label>
              </div>
            </Col>
          </Row>
        </Space>
      </section>

      <section aria-label="充值地址明细" style={{ width: '100%' }}>
        <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
          <Row align="middle" gutter={[16, 16]} justify="space-between" type="flex" style={{ width: '100%' }}>
            <Col>
              <strong>地址明细</strong>
            </Col>
            <Col>
              <Button onClick={appendEntry} theme="borderless" type="primary">
                新增一行
              </Button>
            </Col>
          </Row>
          {values.entries.map((entry, index) => (
            <Card aria-label={`充值地址行 ${index + 1}`} bordered key={index}>
              <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
                <Row align="middle" gutter={[16, 16]} justify="space-between" type="flex" style={{ width: '100%' }}>
                  <Col>
                    <strong>地址 {index + 1}</strong>
                  </Col>
                  <Col>
                    <Button disabled={values.entries.length <= 1} onClick={() => removeEntry(index)} theme="borderless" type="danger">
                      删除本行
                    </Button>
                  </Col>
                </Row>
                <Row gutter={[16, 16]}>
                  <Col xs={24} md={14}>
                    <div className="admin-action-form">
                      <label>
                        充值地址
                        <AdminTextInput ariaLabel="充值地址" value={entry.address} onChange={(address) => updateEntry(index, { ...entry, address })} placeholder="0x... / T... / bc1... / ..." />
                      </label>
                    </div>
                  </Col>
                  <Col xs={24} md={10}>
                    <div className="admin-action-form">
                      <label>
                        Memo / Tag
                        <AdminTextInput ariaLabel="Memo / Tag" value={entry.memo} onChange={(memo) => updateEntry(index, { ...entry, memo })} />
                      </label>
                    </div>
                  </Col>
                  <Col span={24}>
                    <div className="admin-action-form">
                      <label>
                        备注
                        <AdminTextArea ariaLabel="备注" autosize value={entry.remark} onChange={(remark) => updateEntry(index, { ...entry, remark })} />
                      </label>
                    </div>
                  </Col>
                </Row>
              </Space>
            </Card>
          ))}
        </Space>
      </section>
    </Space>
  );
}

function DepositAddressPoolFields({
  assetLoading,
  assetOptions,
  networkConfigLoading,
  networkConfigs,
  onChange,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  networkConfigLoading: boolean;
  networkConfigs: DepositNetworkConfigOption[];
  onChange: (values: DepositAddressPoolValues) => void;
  values: DepositAddressPoolValues;
}) {
  const currentNetworkConfig = depositNetworkConfigForNetwork(networkConfigs, values.network);
  const filteredAssetOptions = depositAssetOptionsForNetwork(assetOptions, currentNetworkConfig);
  const updateNetwork = (network: string) => {
    const nextConfig = depositNetworkConfigForNetwork(networkConfigs, network);
    onChange({
      ...values,
      network,
      addressGroupCode: nextConfig?.addressGroupCode || defaultDepositAddressGroupCode(network),
      assetSymbols: normalizeSelectedDepositAssetsForNetwork(values.assetSymbols, nextConfig)
    });
  };
  return (
    <div className="admin-action-form">
      <label>
        网络
        <AdminSelect ariaLabel="网络" loading={networkConfigLoading} onChange={updateNetwork} optionList={depositNetworkSelectOptions(networkConfigs)} value={values.network} />
      </label>
      <label>地址集合编号<AdminTextInput ariaLabel="地址集合编号" readOnly value={values.addressGroupCode} onChange={() => undefined} /></label>
      <label>充值地址<AdminTextInput ariaLabel="充值地址" value={values.address} onChange={(address) => onChange({ ...values, address })} placeholder="0x... / T... / bc1... / ..." /></label>
      <AssetSymbolMultiSelect
        label="限定资产"
        loading={assetLoading}
        options={filteredAssetOptions}
        value={values.assetSymbols}
        onChange={(assetSymbols) => onChange({ ...values, assetSymbols })}
      />
      <label>
        状态
        <AdminSelect ariaLabel="状态" onChange={(status) => onChange({ ...values, status })} optionList={depositAddressStatusOptions} value={values.status} />
      </label>
      <label>Memo / Tag<AdminTextInput ariaLabel="Memo / Tag" value={values.memo} onChange={(memo) => onChange({ ...values, memo })} /></label>
      <label>备注<AdminTextArea ariaLabel="备注" autosize value={values.remark} onChange={(remark) => onChange({ ...values, remark })} /></label>
    </div>
  );
}

function DepositNetworkConfigFields({
  assetLoading,
  assetOptions,
  onChange,
  values
}: {
  assetLoading: boolean;
  assetOptions: AssetOption[];
  onChange: (values: DepositNetworkConfigValues) => void;
  values: DepositNetworkConfigValues;
}) {
  return (
    <div className="admin-action-form">
      <label>
        网络
        <AdminSelect ariaLabel="网络" onChange={(network) => onChange({ ...values, network })} optionList={depositNetworkOptions} value={values.network} />
      </label>
      <label>显示名称<AdminTextInput ariaLabel="显示名称" value={values.displayName} onChange={(displayName) => onChange({ ...values, displayName })} /></label>
      <label>地址集合编号<AdminTextInput ariaLabel="地址集合编号" value={values.addressGroupCode} onChange={(addressGroupCode) => onChange({ ...values, addressGroupCode })} placeholder="A / EVM / BTC" /></label>
      <label>地址集合名称<AdminTextInput ariaLabel="地址集合名称" value={values.addressGroupName} onChange={(addressGroupName) => onChange({ ...values, addressGroupName })} placeholder="EVM / Bitcoin / Tron" /></label>
      <AssetSymbolMultiSelect
        label="支持充值币种"
        loading={assetLoading}
        options={assetOptions}
        value={values.assetSymbols}
        onChange={(assetSymbols) => onChange({ ...values, assetSymbols })}
      />
      <label>
        状态
        <AdminSelect ariaLabel="状态" onChange={(status) => onChange({ ...values, status })} optionList={statusOptions} value={values.status} />
      </label>
      <label>排序<AdminTextInput ariaLabel="排序" type="number" value={values.sortOrder} onChange={(sortOrder) => onChange({ ...values, sortOrder })} /></label>
    </div>
  );
}

export function CreateDepositNetworkConfigAction({ onCreated }: CreateActionProps = {}) {
  const [config, setConfig] = useState<DepositNetworkConfigValues>(initialDepositNetworkConfig);
  const { assetLoading, assetOptions } = useAssetOptions();

  return (
    <FormModal actionText="新增充值网络配置" size="medium" title="新增充值网络配置">
      {({ close }) => (
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <DepositNetworkConfigFields assetLoading={assetLoading} assetOptions={assetOptions} values={config} onChange={setConfig} />
          <Row justify="end" type="flex" style={{ width: '100%' }}>
            <Col>
              <ConfirmAction
                actionText="提交新增"
                disabled={!isDepositNetworkConfigSubmittable(config)}
                title="确认新增充值网络配置"
                onConfirm={async (reason) => {
                  await submitAction('新增充值网络配置', () =>
                    apiRequest('/admin/api/v1/deposit-network-configs', {
                      method: 'POST',
                      body: JSON.stringify(depositNetworkConfigRequestBody(config, reason))
                    })
                  );
                  completeCreate(close, onCreated, () => setConfig(initialDepositNetworkConfig));
                }}
              />
            </Col>
          </Row>
        </Space>
      )}
    </FormModal>
  );
}

export function DepositNetworkConfigRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const configId = recordString(record, 'id');
  const [config, setConfig] = useState<DepositNetworkConfigValues>(() => depositNetworkConfigFromRecord(record));
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);

  return (
    <>
      <Button disabled={!configId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改充值网络配置" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <DepositNetworkConfigFields assetLoading={assetLoading} assetOptions={assetOptions} values={config} onChange={setConfig} />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isDepositNetworkConfigSubmittable(config)}
              title="确认修改充值网络配置"
              onConfirm={async (reason) => {
                await submitAction('修改充值网络配置', () =>
                  apiRequest(`/admin/api/v1/deposit-network-configs/${configId}`, {
                    method: 'PATCH',
                    body: JSON.stringify(depositNetworkConfigRequestBody(config, reason))
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

export function CreateDepositAddressPoolAction({ onCreated }: CreateActionProps = {}) {
  const [addressPool, setAddressPool] = useState(createInitialDepositAddressPoolCreate);
  const { assetLoading, assetOptions } = useAssetOptions();
  const { networkConfigLoading, networkConfigs } = useDepositNetworkConfigOptions();

  useEffect(() => {
    if (networkConfigs.length === 0) {
      return;
    }
    setAddressPool((current) => {
      const addressGroupCode = depositAddressGroupForNetwork(networkConfigs, current.network, current.addressGroupCode);
      if (addressGroupCode === current.addressGroupCode) {
        return current;
      }
      return { ...current, addressGroupCode };
    });
  }, [networkConfigs]);

  return (
    <FormModal actionText="添加充值地址" size="extra-wide" title="添加充值地址">
      {({ close }) => (
        <Space align="start" spacing={20} vertical style={{ width: '100%' }}>
          <DepositAddressPoolCreateFields
            assetLoading={assetLoading}
            assetOptions={assetOptions}
            networkConfigLoading={networkConfigLoading}
            networkConfigs={networkConfigs}
            values={addressPool}
            onChange={setAddressPool}
          />
          <Row justify="end" type="flex" style={{ width: '100%' }}>
            <Col>
            <ConfirmAction
              actionText="提交添加"
              disabled={!isDepositAddressPoolCreateSubmittable(addressPool)}
              title="确认添加充值地址"
              onConfirm={async (reason) => {
                await submitAction('添加充值地址', () =>
                  apiRequest('/admin/api/v1/deposit-address-pool/batch', {
                    method: 'POST',
                    body: JSON.stringify(depositAddressPoolBatchRequestBody(addressPool, reason))
                  })
                );
                completeCreate(close, onCreated, () => setAddressPool(createInitialDepositAddressPoolCreate()));
              }}
            />
            </Col>
          </Row>
        </Space>
      )}
    </FormModal>
  );
}

function DepositAddressPoolEditAction({ addressId, helpers, record }: { addressId: string; helpers: RowActionHelpers; record: ApiRecord }) {
  const [config, setConfig] = useState<DepositAddressPoolValues>(() => depositAddressPoolFromRecord(record));
  const [visible, setVisible] = useState(false);
  const assigned = recordString(record, 'status') === 'assigned';
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { networkConfigLoading, networkConfigs } = useDepositNetworkConfigOptions(visible);

  return (
    <>
      <Button disabled={!addressId || assigned} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改充值地址" visible={visible} {...createModalProps('medium')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <DepositAddressPoolFields
              assetLoading={assetLoading}
              assetOptions={assetOptions}
              networkConfigLoading={networkConfigLoading}
              networkConfigs={networkConfigs}
              values={config}
              onChange={setConfig}
            />
            <ConfirmAction
              actionText="提交修改"
              disabled={!isDepositAddressPoolSubmittable(config)}
              title="确认修改充值地址"
              onConfirm={async (reason) => {
                await submitAction('修改充值地址', () =>
                  apiRequest(`/admin/api/v1/deposit-address-pool/${addressId}`, {
                    method: 'PATCH',
                    body: JSON.stringify(depositAddressPoolRequestBody(config, reason))
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

export function DepositAddressPoolRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const addressId = recordString(record, 'id');
  const assigned = recordString(record, 'status') === 'assigned';

  return (
    <>
      <Button disabled={!addressId} onClick={() => openRecordDetail('/admin/api/v1/deposit-address-pool', addressId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <DepositAddressPoolEditAction addressId={addressId} helpers={helpers} record={record} />
      {assigned ? (
        <ConfirmAction
          actionText="回收"
          disabled={!addressId}
          title="回收充值地址"
          onConfirm={async (reason) => {
            await submitAction('回收充值地址', () =>
              apiRequest(`/admin/api/v1/deposit-address-pool/${addressId}/reclaim`, {
                method: 'POST',
                body: JSON.stringify({ reason })
              })
            );
            helpers.reload();
          }}
        />
      ) : null}
    </>
  );
}

const withdrawalAdminEndpoint = '/admin/api/v1/wallet/withdrawals';

function BroadcastWithdrawalAction({ helpers, withdrawalId }: { helpers: RowActionHelpers; withdrawalId: string }) {
  const [values, setValues] = useState({ txHash: '', blockHeight: '', confirmations: '' });
  const [submitting, setSubmitting] = useState(false);
  const [visible, setVisible] = useState(false);
  const submittable =
    Boolean(values.txHash.trim()) &&
    (!values.blockHeight.trim() || isNonNegativeIntegerInput(values.blockHeight)) &&
    (!values.confirmations.trim() || isNonNegativeIntegerInput(values.confirmations));

  async function handleConfirm() {
    setSubmitting(true);
    try {
      await submitAction('标记提现广播', () =>
        apiRequest(`${withdrawalAdminEndpoint}/${withdrawalId}/broadcast`, {
          method: 'POST',
          body: JSON.stringify({
            tx_hash: requiredString(values.txHash, '交易哈希'),
            block_height: values.blockHeight.trim() ? requiredNonNegativeInteger(values.blockHeight, '区块高度') : undefined,
            confirmations: values.confirmations.trim() ? requiredNonNegativeInteger(values.confirmations, '确认数') : undefined
          })
        })
      );
      setVisible(false);
      helpers.reload();
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <>
      <Button disabled={!withdrawalId} onClick={() => setVisible(true)} size="small" theme="borderless">
        标记广播
      </Button>
      <Modal
        confirmLoading={submitting}
        motion={false}
        okButtonProps={{ 'aria-label': '确认广播', disabled: !submittable }}
        okText="确认广播"
        onCancel={() => setVisible(false)}
        onOk={handleConfirm}
        title="标记链上广播"
        visible={visible}
      >
        <div className="admin-action-form">
          <label>交易哈希<AdminTextInput ariaLabel="交易哈希" value={values.txHash} onChange={(txHash) => setValues({ ...values, txHash })} placeholder="0x..." /></label>
          <label>区块高度<AdminTextInput ariaLabel="区块高度" value={values.blockHeight} onChange={(blockHeight) => setValues({ ...values, blockHeight })} placeholder="选填" /></label>
          <label>确认数<AdminTextInput ariaLabel="确认数" value={values.confirmations} onChange={(confirmations) => setValues({ ...values, confirmations })} placeholder="选填，默认 0" /></label>
        </div>
      </Modal>
    </>
  );
}

export function WithdrawalRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const withdrawalId = recordString(record, 'id');
  const status = recordString(record, 'status');
  const transition = async (label: string, action: string, body: Record<string, unknown>) => {
    await submitAction(label, () =>
      apiRequest(`${withdrawalAdminEndpoint}/${withdrawalId}/${action}`, { method: 'POST', body: JSON.stringify(body) })
    );
    helpers.reload();
  };

  return (
    <>
      <Button disabled={!withdrawalId} onClick={() => helpers.openDetail({ title: '提现详情', data: record })} size="small" theme="borderless">
        查看详情
      </Button>
      {status === 'pending_review' ? (
        <ConfirmAction actionText="通过" disabled={!withdrawalId} title="确认通过提现审核" onConfirm={(reason) => transition('通过提现审核', 'approve', { reason })} />
      ) : null}
      {status === 'pending_review' || status === 'approved' ? (
        <ConfirmAction actionText="驳回" disabled={!withdrawalId} title="确认驳回提现" onConfirm={(reason) => transition('驳回提现', 'reject', { reason })} />
      ) : null}
      {status === 'approved' || status === 'broadcasting' ? <BroadcastWithdrawalAction helpers={helpers} withdrawalId={withdrawalId} /> : null}
      {status === 'broadcasted' || status === 'manual_review' ? (
        <Popconfirm content="确认该提现已在链上到账？" okText="确认到账" onConfirm={() => transition('确认提现到账', 'confirm', {})} title="确认提现到账">
          <Button disabled={!withdrawalId} size="small" theme="borderless">
            确认到账
          </Button>
        </Popconfirm>
      ) : null}
      {status === 'approved' || status === 'broadcasting' ? (
        <ConfirmAction actionText="标记失败" disabled={!withdrawalId} title="确认标记提现失败" onConfirm={(reason) => transition('标记提现失败', 'fail', { reason })} />
      ) : null}
    </>
  );
}

export function DepositRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const depositId = recordString(record, 'id');
  const status = recordString(record, 'status');

  return (
    <>
      <Button disabled={!depositId} onClick={() => helpers.openDetail({ title: '充值详情', data: record })} size="small" theme="borderless">
        查看详情
      </Button>
      {status === 'credited' ? (
        <ConfirmAction
          actionText="冲正"
          disabled={!depositId}
          title="确认冲正充值"
          onConfirm={async (reason) => {
            await submitAction('冲正充值', () =>
              apiRequest(`/admin/api/v1/wallet/deposits/${depositId}/reverse`, { method: 'POST', body: JSON.stringify({ reason }) })
            );
            helpers.reload();
          }}
        />
      ) : null}
    </>
  );
}

export function QuickRechargeOrderRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const orderId = recordString(record, 'order_id');
  const status = recordString(record, 'status');
  const canDelete = status !== 'paid';

  return (
    <>
      <Button disabled={!orderId} onClick={() => helpers.openDetail({ title: '快速充值订单详情', data: record })} size="small" theme="borderless">
        查看详情
      </Button>
      <ConfirmAction
        actionText="删除"
        disabled={!orderId || !canDelete}
        title="确认删除快速充值订单"
        onConfirm={async (reason) => {
          await submitAction('删除快速充值订单', () =>
            apiRequest(`/admin/api/v1/quick-recharge/orders/${encodeURIComponent(orderId)}`, {
              method: 'DELETE',
              body: JSON.stringify({ reason })
            })
          );
          helpers.reload();
        }}
      />
    </>
  );
}

export function CreateAssetAction({ onCreated }: CreateActionProps = {}) {
  const [asset, setAsset] = useState(initialAsset);

  return (
    <FormModal actionText="添加资产" title="添加资产">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div className="admin-action-form">
            <label>资产符号<AdminTextInput ariaLabel="资产符号" value={asset.symbol} onChange={(symbol) => setAsset({ ...asset, symbol })} placeholder="BTC" /></label>
            <label>资产名称<AdminTextInput ariaLabel="资产名称" value={asset.name} onChange={(name) => setAsset({ ...asset, name })} placeholder="Bitcoin" /></label>
            <AdminImageUpload label="资产 Logo" value={asset.logoUrl} variant="avatar" onChange={(logoUrl) => setAsset({ ...asset, logoUrl })} />
            <label>资产精度<AdminTextInput ariaLabel="资产精度" value={asset.precisionScale} onChange={(precisionScale) => setAsset({ ...asset, precisionScale })} /></label>
            <label>资产类型<AssetTypeSelect value={asset.assetType} onChange={(assetType) => setAsset({ ...asset, assetType })} /></label>
            <label>初始状态<AssetStatusSelect value={asset.status} onChange={(status) => setAsset({ ...asset, status })} /></label>
            <AdminSwitch checked={asset.depositEnabled} label="支持充值" onChange={(depositEnabled) => setAsset({ ...asset, depositEnabled })} />
            <AdminSwitch checked={asset.withdrawEnabled} label="支持提现" onChange={(withdrawEnabled) => setAsset({ ...asset, withdrawEnabled })} />
            <label>最小充值数量<AdminTextInput ariaLabel="最小充值数量" value={asset.minDepositAmount} onChange={(minDepositAmount) => setAsset({ ...asset, minDepositAmount })} /></label>
            <label>充值手续费<AdminTextInput ariaLabel="充值手续费" value={asset.depositFee} onChange={(depositFee) => setAsset({ ...asset, depositFee })} /></label>
            <label>提现手续费<AdminTextInput ariaLabel="提现手续费" value={asset.withdrawFee} onChange={(withdrawFee) => setAsset({ ...asset, withdrawFee })} /></label>
            <AssetWithdrawFeeTiersEditor values={asset.withdrawFeeTiers} onChange={(withdrawFeeTiers) => setAsset({ ...asset, withdrawFeeTiers })} />
          </div>
          <ConfirmAction
            actionText="提交添加资产"
            disabled={!isAssetCreatable(asset)}
            title="确认添加资产"
            onConfirm={async (reason) => {
              await submitAction('添加资产', () =>
                apiRequest('/admin/api/v1/assets', {
                  method: 'POST',
                  body: JSON.stringify({
                    symbol: requiredString(asset.symbol, '资产符号'),
                    name: requiredString(asset.name, '资产名称'),
                    logo_url: optionalString(asset.logoUrl),
                    precision_scale: requiredNonNegativeInteger(asset.precisionScale, '资产精度'),
                    asset_type: asset.assetType,
                    status: asset.status,
                    deposit_enabled: asset.depositEnabled,
                    withdraw_enabled: asset.withdrawEnabled,
                    min_deposit_amount: requiredNonNegativeDecimal(asset.minDepositAmount, '最小充值数量'),
                    deposit_fee: requiredNonNegativeDecimal(asset.depositFee, '充值手续费'),
                    withdraw_fee: requiredNonNegativeDecimal(asset.withdrawFee, '提现手续费'),
                    withdraw_fee_tiers: withdrawFeeTierPayload(asset.withdrawFeeTiers),
                    reason
                  })
                })
              );
              completeCreate(close, onCreated, () => setAsset(initialAsset));
            }}
          />
        </Space>
      </Card>
      )}
    </FormModal>
  );
}
