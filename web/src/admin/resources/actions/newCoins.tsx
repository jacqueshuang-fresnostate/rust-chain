import { Card, Space } from '@douyinfe/semi-ui';
import { useState } from 'react';

import { apiRequest } from '../../../api/client';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminSelect, AdminTextInput, type SemiSelectOption } from '../../../shared/SemiFormControls';
import {
  type AssetOption,
  AssetSelect,
  BooleanSelect,
  type CreateActionProps,
  FormModal,
  booleanFromSelect,
  completeCreate,
  requiredPositiveInteger,
  requiredString,
  submitAction,
  useAssetOptions
} from './shared';

type NewCoinProjectValues = {
  assetId: string;
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

function isNewCoinProjectCreatable(values: NewCoinProjectValues): boolean {
  return Boolean(values.assetId.trim() && values.symbol.trim() && values.lifecycleStatus.trim() && values.totalSupply.trim() && values.issuePrice.trim() && values.unlockType.trim());
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
    setProject({ ...project, assetId, symbol: selectedAsset?.symbol || project.symbol });
  };
  const selectProjectSymbol = (symbol: string) => {
    const selectedAsset = assetOptions.find((asset) => asset.symbol === symbol);
    setProject({ ...project, assetId: selectedAsset?.id || project.assetId, symbol });
  };

  return (
    <FormModal actionText="添加新币项目" size="extra-wide" title="添加新币项目">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <div className="admin-action-form">
            <AssetSelect label="项目资产" loading={assetLoading} options={assetOptions} value={project.assetId} onChange={selectProjectAsset} />
            <AssetSymbolSelect label="项目符号" loading={assetLoading} options={assetOptions} value={project.symbol} onChange={selectProjectSymbol} />
            <label>
              生命周期
              <AdminSelect
                ariaLabel="生命周期"
                onChange={(lifecycleStatus) => setProject({ ...project, lifecycleStatus })}
                optionList={newCoinLifecycleOptions}
                value={project.lifecycleStatus}
              />
            </label>
            <label>发行总量<AdminTextInput ariaLabel="发行总量" value={project.totalSupply} onChange={(totalSupply) => setProject({ ...project, totalSupply })} /></label>
            <label>发行价<AdminTextInput ariaLabel="发行价" value={project.issuePrice} onChange={(issuePrice) => setProject({ ...project, issuePrice })} /></label>
            <label>
              解禁类型
              <AdminSelect
                ariaLabel="解禁类型"
                onChange={(unlockType) => setProject({ ...project, unlockType })}
                optionList={newCoinUnlockTypeOptions}
                value={project.unlockType}
              />
            </label>
            {project.unlockType === 'immediate_on_listing' ? <label>上市时间<AdminTextInput ariaLabel="上市时间" value={project.listedAt} onChange={(listedAt) => setProject({ ...project, listedAt })} /></label> : null}
            {project.unlockType === 'fixed_time' ? <label>固定解禁时间<AdminTextInput ariaLabel="固定解禁时间" value={project.fixedUnlockAt} onChange={(fixedUnlockAt) => setProject({ ...project, fixedUnlockAt })} /></label> : null}
            {project.unlockType === 'relative_period' ? <label>相对解禁秒数<AdminTextInput ariaLabel="相对解禁秒数" value={project.relativeUnlockSeconds} onChange={(relativeUnlockSeconds) => setProject({ ...project, relativeUnlockSeconds })} /></label> : null}
            <label>启用解禁矿工费<BooleanSelect label="启用解禁矿工费" value={project.unlockFeeEnabled} onChange={(unlockFeeEnabledValue) => setProject({ ...project, unlockFeeEnabled: unlockFeeEnabledValue })} /></label>
            {unlockFeeEnabled ? (
              <>
                <label>解禁费率<AdminTextInput ariaLabel="解禁费率" value={project.unlockFeeRate} onChange={(unlockFeeRate) => setProject({ ...project, unlockFeeRate })} /></label>
                <label>解禁费计费基准<AdminTextInput ariaLabel="解禁费计费基准" value={project.unlockFeeBasis} onChange={(unlockFeeBasis) => setProject({ ...project, unlockFeeBasis })} /></label>
                <AssetSelect label="解禁费资产" loading={assetLoading} options={assetOptions} value={project.unlockFeeAsset} onChange={(unlockFeeAsset) => setProject({ ...project, unlockFeeAsset })} />
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
                symbol: requiredString(project.symbol, '项目符号'),
                lifecycle_status: requiredString(project.lifecycleStatus, '生命周期'),
                total_supply: requiredString(project.totalSupply, '发行总量'),
                issue_price: requiredString(project.issuePrice, '发行价'),
                unlock_type: requiredString(project.unlockType, '解禁类型'),
                unlock_fee_enabled: unlockFeeEnabled,
                reason
              };
              if (project.unlockType === 'immediate_on_listing') {
                body.listed_at = requiredPositiveInteger(project.listedAt, '上市时间');
              }
              if (project.unlockType === 'fixed_time') {
                body.fixed_unlock_at = requiredPositiveInteger(project.fixedUnlockAt, '固定解禁时间');
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
