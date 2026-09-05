import { Button, Card, Space } from '@douyinfe/semi-ui';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminSelect, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import { AdminRequestActionBoundary } from '../../access';
import { isValidNewCoinLocalDateTime, requiredNewCoinLocalDateTimeMillis } from '../../newCoinDateTime';
import {
  type AssetOption,
  AssetSelect,
  BooleanSelect,
  type CreateActionProps,
  FormModal,
  type RowActionHelpers,
  booleanFromSelect,
  completeCreate,
  recordString,
  requiredPositiveInteger,
  requiredString,
  submitAction,
  useAssetOptions
} from './shared';

export function newCoinProjectActionsPath(projectId: string): string {
  return `/admin/new-coins/projects/${encodeURIComponent(projectId)}`;
}

export function NewCoinProjectRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const navigate = useNavigate();
  const projectId = recordString(record, 'id');
  const lifecycleStatus = recordString(record, 'lifecycle_status').toLowerCase();
  const projectStatus = recordString(record, 'status').toLowerCase();
  const symbol = recordString(record, 'symbol');
  const projectIdentity = symbol ? `${symbol}（ID: ${projectId || '-'}）` : `ID: ${projectId || '-'}`;
  const canStartSubscription = Boolean(projectId) && lifecycleStatus === 'preheat' && projectStatus === 'active';

  return (
    <>
      {lifecycleStatus === 'preheat' ? (
        <AdminRequestActionBoundary endpoint={`/admin/api/v1/new-coins/${projectId}/lifecycle`} method="PATCH">
          <ConfirmAction
            actionAriaLabel={`开始申购 ${projectIdentity}`}
            actionText="开始申购"
            disabled={!canStartSubscription}
            title={`确认 ${projectIdentity} 开始申购`}
            onConfirm={async (reason) => {
              await submitAction('开始新币申购', () =>
                apiRequest(`/admin/api/v1/new-coins/${requiredPositiveInteger(projectId, '项目ID')}/lifecycle`, {
                  method: 'PATCH',
                  body: JSON.stringify({ lifecycle_status: 'subscription', reason })
                })
              );
              helpers.reload();
            }}
          />
        </AdminRequestActionBoundary>
      ) : null}
      <Button
        aria-label={`配置新币项目 ${projectIdentity}`}
        disabled={!projectId}
        onClick={() => navigate(newCoinProjectActionsPath(projectId))}
        size="small"
        theme="borderless"
      >
        项目中心
      </Button>
    </>
  );
}

type NewCoinProjectValues = {
  assetId: string;
  quoteAssetId: string;
  symbol: string;
  lifecycleStatus: string;
  totalSupply: string;
  issuePrice: string;
  unlockType: string;
  listedAt: string;
  fixedUnlockAt: string;
  relativeUnlockSeconds: string;
  unlockFeeEnabled: string;
  unlockFeeRate: string;
  unlockFeeBasis: string;
  unlockFeeAsset: string;
};

const initialNewCoinProject: NewCoinProjectValues = {
  assetId: '',
  quoteAssetId: '',
  symbol: '',
  lifecycleStatus: 'preheat',
  totalSupply: '',
  issuePrice: '',
  unlockType: 'fixed_time',
  listedAt: '',
  fixedUnlockAt: '',
  relativeUnlockSeconds: '',
  unlockFeeEnabled: 'false',
  unlockFeeRate: '',
  unlockFeeBasis: 'market_value',
  unlockFeeAsset: ''
};

const newCoinLifecycleOptions: SemiSelectOption[] = [
  { value: 'preheat', label: '预热' },
  { value: 'subscription', label: '申购中' },
  { value: 'distribution', label: '分发中' },
  { value: 'listed', label: '已上市' }
];

const newCoinUnlockTypeOptions: SemiSelectOption[] = [
  { value: 'immediate_on_listing', label: '上市即解禁' },
  { value: 'fixed_time', label: '固定时间解禁' },
  { value: 'relative_period', label: '相对周期解禁' }
];

const newCoinUnlockFeeBasisOptions: SemiSelectOption[] = [
  { value: 'market_value', label: '按解禁市值计费' },
  { value: 'profit', label: '按解禁收益计费' }
];

function isPositiveIntegerInput(value: string): boolean {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0;
}

function hasValidNewCoinUnlockSchedule(values: NewCoinProjectValues): boolean {
  if (values.unlockType === 'immediate_on_listing') {
    return isValidNewCoinLocalDateTime(values.listedAt);
  }
  if (values.unlockType === 'fixed_time') {
    return isValidNewCoinLocalDateTime(values.fixedUnlockAt);
  }
  if (values.unlockType === 'relative_period') {
    return isPositiveIntegerInput(values.relativeUnlockSeconds);
  }
  return false;
}

export function isNewCoinProjectCreatable(values: NewCoinProjectValues): boolean {
  const assetId = Number(values.assetId);
  const quoteAssetId = Number(values.quoteAssetId);
  return Boolean(
    Number.isInteger(assetId) &&
    assetId > 0 &&
    Number.isInteger(quoteAssetId) &&
    quoteAssetId > 0 &&
    assetId !== quoteAssetId &&
    values.symbol.trim() &&
    values.lifecycleStatus.trim() &&
    values.totalSupply.trim() &&
    values.issuePrice.trim() &&
    values.unlockType.trim() &&
    hasValidNewCoinUnlockSchedule(values)
  );
}

function AssetSymbolSelect({
  label,
  loading,
  onChange,
  options,
  value
}: {
  label: string;
  loading: boolean;
  onChange: (value: string) => void;
  options: AssetOption[];
  value: string;
}) {
  const symbolOptions = options
    .filter((asset) => asset.symbol)
    .map((asset) => ({ value: asset.symbol, label: asset.label }));

  return (
    <label>
      {label}
      <AdminSelect
        ariaLabel={label}
        disabled={loading}
        loading={loading}
        onChange={onChange}
        optionList={symbolOptions}
        placeholder={loading ? '加载资产中...' : '请选择项目符号'}
        value={value}
      />
    </label>
  );
}

function optionalPositiveInteger(value: string, label: string): number | undefined {
  const trimmed = value.trim();
  return trimmed ? requiredPositiveInteger(trimmed, label) : undefined;
}

export function CreateNewCoinProjectAction({ onCreated }: CreateActionProps = {}) {
  const [project, setProject] = useState(initialNewCoinProject);
  const { assetLoading, assetOptions } = useAssetOptions();
  const unlockFeeEnabled = booleanFromSelect(project.unlockFeeEnabled);
  const selectProjectAsset = (assetId: string) => {
    const selectedAsset = assetOptions.find((asset) => asset.id === assetId);
    setProject((current) => ({
      ...current,
      assetId,
      quoteAssetId: current.quoteAssetId === assetId ? '' : current.quoteAssetId,
      symbol: selectedAsset?.symbol || current.symbol
    }));
  };
  const selectProjectSymbol = (symbol: string) => {
    const selectedAsset = assetOptions.find((asset) => asset.symbol === symbol);
    setProject((current) => {
      const assetId = selectedAsset?.id || current.assetId;
      return {
        ...current,
        assetId,
        quoteAssetId: current.quoteAssetId === assetId ? '' : current.quoteAssetId,
        symbol
      };
    });
  };
  const quoteAssetOptions = assetOptions.filter((asset) => asset.id !== project.assetId);

  return (
    <FormModal actionText="添加新币项目" size="extra-wide" title="添加新币项目">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div className="admin-action-form">
            <AssetSelect label="项目资产" loading={assetLoading} options={assetOptions} value={project.assetId} onChange={selectProjectAsset} />
            <AssetSelect
              label="计价资产"
              loading={assetLoading}
              options={quoteAssetOptions}
              value={project.quoteAssetId}
              onChange={(quoteAssetId) => setProject((current) => ({
                ...current,
                quoteAssetId: current.assetId === quoteAssetId ? '' : quoteAssetId
              }))}
            />
            <AssetSymbolSelect label="项目符号" loading={assetLoading} options={assetOptions} value={project.symbol} onChange={selectProjectSymbol} />
            <label>
              生命周期
              <AdminSelect
                ariaLabel="生命周期"
                onChange={(lifecycleStatus) => setProject((current) => ({ ...current, lifecycleStatus }))}
                optionList={newCoinLifecycleOptions}
                value={project.lifecycleStatus}
              />
            </label>
            <label>发行总量<AdminTextInput ariaLabel="发行总量" value={project.totalSupply} onChange={(totalSupply) => setProject((current) => ({ ...current, totalSupply }))} /></label>
            <label>发行价<AdminTextInput ariaLabel="发行价" value={project.issuePrice} onChange={(issuePrice) => setProject((current) => ({ ...current, issuePrice }))} /></label>
            <label>
              解禁类型
              <AdminSelect
                ariaLabel="解禁类型"
                onChange={(unlockType) => setProject((current) => ({ ...current, unlockType }))}
                optionList={newCoinUnlockTypeOptions}
                value={project.unlockType}
              />
            </label>
            {project.unlockType === 'immediate_on_listing' ? <label>计划上市时间<AdminTextInput ariaLabel="计划上市时间" type="datetime-local" value={project.listedAt} onChange={(listedAt) => setProject((current) => ({ ...current, listedAt }))} /></label> : null}
            {project.unlockType === 'fixed_time' ? <label>固定解禁时间<AdminTextInput ariaLabel="固定解禁时间" type="datetime-local" value={project.fixedUnlockAt} onChange={(fixedUnlockAt) => setProject((current) => ({ ...current, fixedUnlockAt }))} /></label> : null}
            {project.unlockType === 'relative_period' ? <label>相对解禁秒数<AdminTextInput ariaLabel="相对解禁秒数" value={project.relativeUnlockSeconds} onChange={(relativeUnlockSeconds) => setProject((current) => ({ ...current, relativeUnlockSeconds }))} /></label> : null}
            <label>启用解禁矿工费<BooleanSelect label="启用解禁矿工费" value={project.unlockFeeEnabled} onChange={(unlockFeeEnabledValue) => setProject((current) => ({ ...current, unlockFeeEnabled: unlockFeeEnabledValue }))} /></label>
            {unlockFeeEnabled ? (
              <>
                <label>解禁费率<AdminTextInput ariaLabel="解禁费率" value={project.unlockFeeRate} onChange={(unlockFeeRate) => setProject((current) => ({ ...current, unlockFeeRate }))} /></label>
                <label>
                  解禁费计费基准
                  <AdminSelect
                    ariaLabel="解禁费计费基准"
                    onChange={(unlockFeeBasis) => setProject((current) => ({ ...current, unlockFeeBasis }))}
                    optionList={newCoinUnlockFeeBasisOptions}
                    value={project.unlockFeeBasis}
                  />
                </label>
                <AssetSelect label="解禁费资产" loading={assetLoading} options={assetOptions} value={project.unlockFeeAsset} onChange={(unlockFeeAsset) => setProject((current) => ({ ...current, unlockFeeAsset }))} />
              </>
            ) : null}
          </div>
          <ConfirmAction
            actionText="提交添加新币项目"
            disabled={!isNewCoinProjectCreatable(project)}
            title="确认添加新币项目"
            onConfirm={async (reason) => {
              const body: Record<string, unknown> = {
                asset_id: requiredPositiveInteger(project.assetId, '项目资产'),
                quote_asset_id: requiredPositiveInteger(project.quoteAssetId, '计价资产'),
                symbol: requiredString(project.symbol, '项目符号'),
                lifecycle_status: requiredString(project.lifecycleStatus, '生命周期'),
                total_supply: requiredString(project.totalSupply, '发行总量'),
                issue_price: requiredString(project.issuePrice, '发行价'),
                unlock_type: requiredString(project.unlockType, '解禁类型'),
                unlock_fee_enabled: unlockFeeEnabled,
                reason
              };
              if (project.unlockType === 'immediate_on_listing') {
                body.listed_at = requiredNewCoinLocalDateTimeMillis(project.listedAt, '计划上市时间');
              }
              if (project.unlockType === 'fixed_time') {
                body.fixed_unlock_at = requiredNewCoinLocalDateTimeMillis(project.fixedUnlockAt, '固定解禁时间');
              }
              if (project.unlockType === 'relative_period') {
                body.relative_unlock_seconds = requiredPositiveInteger(project.relativeUnlockSeconds, '相对解禁秒数');
              }
              if (unlockFeeEnabled) {
                body.unlock_fee_rate = requiredString(project.unlockFeeRate, '解禁费率');
                body.unlock_fee_basis = requiredString(project.unlockFeeBasis, '解禁费计费基准');
                body.unlock_fee_asset = optionalPositiveInteger(project.unlockFeeAsset, '解禁费资产');
              }

              await submitAction('添加新币项目', () =>
                apiRequest('/admin/api/v1/new-coins', {
                  method: 'POST',
                  body: JSON.stringify(body)
                })
              );
              completeCreate(close, onCreated, () => setProject(initialNewCoinProject));
            }}
          />
        </Space>
      </Card>
      )}
    </FormModal>
  );
}
