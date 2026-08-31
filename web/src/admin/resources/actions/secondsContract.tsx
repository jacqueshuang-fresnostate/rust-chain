import { Button, Card, SideSheet, Space, Typography } from '@douyinfe/semi-ui';
import { type KeyboardEvent, type ReactNode, useId, useRef, useState } from 'react';

import { apiRequest } from '../../../api/client';
import type { ApiRecord } from '../../../api/types';
import { AdminRequestActionBoundary } from '../../access';
import { ConfirmAction } from '../../../shared/ConfirmAction';
import { AdminImageUpload } from '../../../shared/AdminImageUpload';
import { AdminSelect, AdminTextInput } from '../../../shared/SemiFormControls';
import {
  AssetSelect,
  type CreateActionProps,
  FormModal,
  MarketPairSelect,
  type RowActionHelpers,
  completeCreate,
  createModalProps,
  includeCurrentOption,
  nextToggleStatus,
  openRecordDetail,
  optionalString,
  recordString,
  requiredPositiveInteger,
  requiredString,
  statusOptions,
  submitAction,
  toggleActionText,
  useAssetOptions,
  useMarketPairOptions
} from './shared';

const { Title } = Typography;

type SecondsProductValues = {
  logoUrl: string;
  pairId: string;
  stakeAsset: string;
  periods: SecondsProductPeriodValues[];
  status: string;
};

type SecondsProductPeriodValues = {
  durationSeconds: string;
  rowId: string;
  payoutRate: string;
  minStake: string;
  maxStake: string;
};

type SecondsProductTab = 'basic' | 'trade';

let secondsPeriodSequence = 0;

function newSecondsProductPeriod(values: Partial<Omit<SecondsProductPeriodValues, 'rowId'>> = {}, stableId?: string): SecondsProductPeriodValues {
  secondsPeriodSequence += 1;
  return {
    durationSeconds: '',
    payoutRate: '',
    minStake: '',
    maxStake: '',
    ...values,
    rowId: stableId ?? `period-${secondsPeriodSequence}`
  };
}

function newSecondsProduct(): SecondsProductValues {
  return {
    logoUrl: '',
    pairId: '',
    stakeAsset: '',
    periods: [newSecondsProductPeriod()],
    status: 'active'
  };
}

const secondsProductTabs = [
  { itemKey: 'basic', tab: '基础配置' },
  { itemKey: 'trade', tab: '交易参数' }
];

function SecondsProductTabs({
  activeTab,
  children,
  onActiveTabChange
}: {
  activeTab: SecondsProductTab;
  children: ReactNode;
  onActiveTabChange: (tab: SecondsProductTab) => void;
}) {
  const instanceId = useId().replaceAll(':', '');
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const activeIndex = secondsProductTabs.findIndex((tab) => tab.itemKey === activeTab);
  const tabId = (key: string) => `${instanceId}-seconds-tab-${key}`;
  const panelId = (key: string) => `${instanceId}-seconds-panel-${key}`;

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    let nextIndex: number | null = null;
    if (event.key === 'ArrowRight') nextIndex = (index + 1) % secondsProductTabs.length;
    if (event.key === 'ArrowLeft') nextIndex = (index - 1 + secondsProductTabs.length) % secondsProductTabs.length;
    if (event.key === 'Home') nextIndex = 0;
    if (event.key === 'End') nextIndex = secondsProductTabs.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const nextTab = secondsProductTabs[nextIndex].itemKey as SecondsProductTab;
    onActiveTabChange(nextTab);
    tabRefs.current[nextIndex]?.focus();
  };

  return (
    <>
      <div aria-label="秒合约产品配置" className="admin-seconds-product-tabs" role="tablist">
        {secondsProductTabs.map((tab, index) => {
          const selected = index === activeIndex;
          return (
            <button
              aria-controls={panelId(tab.itemKey)}
              aria-selected={selected}
              className={`semi-button semi-button-size-default ${selected ? 'semi-button-primary' : 'semi-button-tertiary'}`}
              id={tabId(tab.itemKey)}
              key={tab.itemKey}
              onClick={() => onActiveTabChange(tab.itemKey as SecondsProductTab)}
              onKeyDown={(event) => onKeyDown(event, index)}
              ref={(element) => {
                tabRefs.current[index] = element;
              }}
              role="tab"
              tabIndex={selected ? 0 : -1}
              type="button"
            >
              {tab.tab}
            </button>
          );
        })}
      </div>
      <div
        aria-labelledby={tabId(activeTab)}
        id={panelId(activeTab)}
        key={activeTab}
        role="tabpanel"
        tabIndex={0}
      >
        {children}
      </div>
    </>
  );
}

function isSecondsProductPeriodSubmittable(period: SecondsProductPeriodValues): boolean {
  return Boolean(period.durationSeconds.trim() && period.payoutRate.trim() && period.minStake.trim());
}

function secondsProductDurationKeys(periods: SecondsProductPeriodValues[]): string[] {
  return periods.map((period) => period.durationSeconds.trim()).filter(Boolean);
}

function isSecondsProductCreatable(values: SecondsProductValues): boolean {
  const durationKeys = secondsProductDurationKeys(values.periods);
  return Boolean(
    values.pairId.trim() &&
      values.stakeAsset.trim() &&
      values.status.trim() &&
      values.periods.length > 0 &&
      values.periods.every(isSecondsProductPeriodSubmittable) &&
      durationKeys.length === new Set(durationKeys).size
  );
}

function secondsProductFromRecord(record: ApiRecord): SecondsProductValues {
  const cycles = Array.isArray(record.cycles) ? record.cycles : [];
  const periods = cycles
    .map((cycle) => {
      if (!cycle || typeof cycle !== 'object') {
        return null;
      }
      const cycleRecord = cycle as ApiRecord;
      return {
        durationSeconds: recordString(cycleRecord, 'duration_seconds'),
        rowId: '',
        payoutRate: recordString(cycleRecord, 'payout_rate'),
        minStake: recordString(cycleRecord, 'min_stake'),
        maxStake: recordString(cycleRecord, 'max_stake')
      };
    })
    .filter((period): period is SecondsProductPeriodValues => Boolean(period?.durationSeconds))
    .map((period, index) => newSecondsProductPeriod(period, `stored-${recordString(cycles[index] as ApiRecord, 'id') || index}`));

  return {
    logoUrl: recordString(record, 'logo_url'),
    pairId: recordString(record, 'pair_id'),
    stakeAsset: recordString(record, 'stake_asset'),
    periods: periods.length
      ? periods
      : [
          newSecondsProductPeriod({
            durationSeconds: recordString(record, 'duration_seconds'),
            payoutRate: recordString(record, 'payout_rate'),
            minStake: recordString(record, 'min_stake'),
            maxStake: recordString(record, 'max_stake')
          })
        ],
    status: recordString(record, 'status') || 'active'
  };
}

function secondsProductRequestBody(values: SecondsProductValues, reason: string) {
  return {
    pair_id: requiredPositiveInteger(values.pairId, '秒合约交易对ID'),
    stake_asset: requiredPositiveInteger(values.stakeAsset, '押注资产ID'),
    logo_url: optionalString(values.logoUrl),
    cycles: values.periods.map((period) => ({
      duration_seconds: requiredPositiveInteger(period.durationSeconds, '周期秒数'),
      payout_rate: requiredString(period.payoutRate, '赔率'),
      min_stake: requiredString(period.minStake, '最小押注'),
      max_stake: optionalString(period.maxStake)
    })),
    status: requiredString(values.status, '状态'),
    reason
  };
}

export function SecondsProductRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const productId = recordString(record, 'id');
  const status = recordString(record, 'status');
  const nextStatus = nextToggleStatus(recordString(record, 'status'));
  const actionText = toggleActionText(nextStatus);

  return (
    <>
      <Button disabled={!productId} onClick={() => openRecordDetail('/admin/api/v1/seconds-contracts/products', productId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <AdminRequestActionBoundary endpoint={`/admin/api/v1/seconds-contracts/products/${productId}`} method="PATCH">
        <SecondsProductEditAction helpers={helpers} productId={productId} record={record} />
        <ConfirmAction
        actionText={actionText}
        disabled={!productId}
        title={`${actionText}秒合约产品`}
        onConfirm={async (reason) => {
          await submitAction(`${actionText}秒合约产品`, () =>
            apiRequest(`/admin/api/v1/seconds-contracts/products/${productId}/status`, {
              method: 'PATCH',
              body: JSON.stringify({ status: nextStatus, reason })
            })
          );
          helpers.reload();
        }}
        />
      </AdminRequestActionBoundary>
      {status === 'disabled' ? (
        <AdminRequestActionBoundary endpoint={`/admin/api/v1/seconds-contracts/products/${productId}`} method="DELETE">
        <ConfirmAction
          actionText="删除"
          disabled={!productId}
          title="确认删除秒合约产品"
          onConfirm={async (reason) => {
            await submitAction('删除秒合约产品', () =>
              apiRequest(`/admin/api/v1/seconds-contracts/products/${productId}`, {
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

function SecondsProductPeriodsEditor({
  onAdd,
  onRemove,
  onUpdate,
  periods
}: {
  onAdd: () => void;
  onRemove: (rowId: string) => void;
  onUpdate: (rowId: string, patch: Partial<SecondsProductPeriodValues>) => void;
  periods: SecondsProductPeriodValues[];
}) {
  const editorId = useId().replaceAll(':', '');

  const removeAndRestoreFocus = (rowId: string, index: number) => {
    const focusTarget = periods[index + 1] ?? periods[index - 1];
    onRemove(rowId);
    globalThis.setTimeout(() => {
      if (focusTarget) document.getElementById(`${editorId}-${focusTarget.rowId}`)?.querySelector<HTMLElement>('input')?.focus();
    }, 0);
  };

  return (
    <Space align="start" spacing={12} vertical style={{ width: '100%' }}>
      <div className="admin-earn-section-header">
        <Title heading={5}>周期配置</Title>
        <Button onClick={onAdd} theme="borderless">
          新增周期
        </Button>
      </div>
      {periods.map((period, index) => (
        <div
          aria-label={`周期 ${index + 1}：${period.durationSeconds.trim() ? `${period.durationSeconds.trim()} 秒` : '未填写秒数'}`}
          className="admin-action-form admin-action-form-wide"
          id={`${editorId}-${period.rowId}`}
          key={period.rowId}
          role="group"
        >
          <label>周期秒数<AdminTextInput ariaLabel="周期秒数" value={period.durationSeconds} onChange={(durationSeconds) => onUpdate(period.rowId, { durationSeconds })} /></label>
          <label>赔率<AdminTextInput ariaLabel="赔率" value={period.payoutRate} onChange={(payoutRate) => onUpdate(period.rowId, { payoutRate })} /></label>
          <label>最小押注<AdminTextInput ariaLabel="最小押注" value={period.minStake} onChange={(minStake) => onUpdate(period.rowId, { minStake })} /></label>
          <label>
            最大押注
            <AdminTextInput ariaLabel="最大押注" placeholder="留空表示无上限" value={period.maxStake} onChange={(maxStake) => onUpdate(period.rowId, { maxStake })} />
          </label>
          <Button
            aria-label={`删除周期 ${index + 1}：${period.durationSeconds.trim() ? `${period.durationSeconds.trim()} 秒` : '未填写秒数'}`}
            disabled={periods.length === 1}
            onClick={() => removeAndRestoreFocus(period.rowId, index)}
            theme="borderless"
          >
            删除周期
          </Button>
        </div>
      ))}
    </Space>
  );
}

function SecondsProductEditAction({ helpers, productId, record }: { helpers: RowActionHelpers; productId: string; record: ApiRecord }) {
  const [config, setConfig] = useState(() => secondsProductFromRecord(record));
  const [activeTab, setActiveTab] = useState<SecondsProductTab>('basic');
  const [visible, setVisible] = useState(false);
  const { assetLoading, assetOptions } = useAssetOptions(visible);
  const { pairLoading, pairOptions } = useMarketPairOptions(visible);
  const pairOptionsWithCurrent = includeCurrentOption(pairOptions, config.pairId, `${recordString(record, 'symbol') || `交易对${config.pairId}`}（ID: ${config.pairId}）`);
  const assetOptionsWithCurrent = includeCurrentOption(
    assetOptions,
    config.stakeAsset,
    `${recordString(record, 'stake_asset_symbol') || `资产${config.stakeAsset}`}（ID: ${config.stakeAsset}）`
  );
  const updatePeriod = (rowId: string, patch: Partial<SecondsProductPeriodValues>) => {
    setConfig((current) => ({
      ...current,
      periods: current.periods.map((period) => (period.rowId === rowId ? { ...period, ...patch } : period))
    }));
  };
  const addPeriod = () => {
    setConfig((current) => ({
      ...current,
      periods: [...current.periods, newSecondsProductPeriod()]
    }));
  };
  const removePeriod = (rowId: string) => {
    setConfig((current) => ({
      ...current,
      periods: current.periods.length > 1 ? current.periods.filter((period) => period.rowId !== rowId) : current.periods
    }));
  };

  return (
    <>
      <Button disabled={!productId} onClick={() => setVisible(true)} size="small" theme="borderless">
        修改
      </Button>
      <SideSheet onCancel={() => setVisible(false)} title="修改秒合约产品" visible={visible} {...createModalProps('wide')}>
        <Card bordered={false}>
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <SecondsProductTabs activeTab={activeTab} onActiveTabChange={setActiveTab}>
              {activeTab === 'basic' ? (
              <div className="admin-action-form admin-action-form-wide">
                <label>产品ID<AdminTextInput ariaLabel="产品ID" readOnly value={productId} onChange={() => undefined} /></label>
                <MarketPairSelect
                  label="秒合约交易对"
                  loading={pairLoading}
                  options={pairOptionsWithCurrent}
                  value={config.pairId}
                  onChange={(pairId) => setConfig({ ...config, pairId })}
                />
                <AssetSelect
                  label="押注资产"
                  loading={assetLoading}
                  options={assetOptionsWithCurrent}
                  value={config.stakeAsset}
                  onChange={(stakeAsset) => setConfig({ ...config, stakeAsset })}
                />
                <AdminImageUpload label="秒合约交易对 Logo" value={config.logoUrl} variant="avatar" onChange={(logoUrl) => setConfig({ ...config, logoUrl })} />
                <label>
                  状态
                  <AdminSelect ariaLabel="状态" onChange={(status) => setConfig({ ...config, status })} optionList={statusOptions} value={config.status} />
                </label>
              </div>
              ) : (
              <SecondsProductPeriodsEditor periods={config.periods} onAdd={addPeriod} onRemove={removePeriod} onUpdate={updatePeriod} />
              )}
            </SecondsProductTabs>
            <div className="admin-action-footer">
              <ConfirmAction
                actionText="提交修改"
                disabled={!isSecondsProductCreatable(config)}
                title="确认修改秒合约产品"
                onConfirm={async (reason) => {
                  await submitAction('修改秒合约产品', () =>
                    apiRequest(`/admin/api/v1/seconds-contracts/products/${productId}`, {
                      method: 'PATCH',
                      body: JSON.stringify(secondsProductRequestBody(config, reason))
                    })
                  );
                  setVisible(false);
                  helpers.reload();
                }}
              />
            </div>
          </Space>
        </Card>
      </SideSheet>
    </>
  );
}

export function SecondsOrderRowActions({ helpers, record }: { helpers: RowActionHelpers; record: ApiRecord }) {
  const orderId = recordString(record, 'id');
  const canSettle = recordString(record, 'status') === 'opened';

  async function settle(result: 'win' | 'loss', reason: string) {
    await submitAction(result === 'win' ? '结算赢' : '结算输', () =>
      apiRequest(`/admin/api/v1/seconds-contracts/orders/${orderId}/settle`, {
        method: 'POST',
        body: JSON.stringify({ result, reason })
      })
    );
    helpers.reload();
  }

  return (
    <>
      <Button disabled={!orderId} onClick={() => openRecordDetail('/admin/api/v1/seconds-contracts/orders', orderId, helpers)} size="small" theme="borderless">
        查看详情
      </Button>
      <AdminRequestActionBoundary endpoint={`/admin/api/v1/seconds-contracts/orders/${orderId}/settle`} method="POST">
        <ConfirmAction actionText="结算赢" disabled={!orderId || !canSettle} title="结算赢" onConfirm={(reason) => settle('win', reason)} />
        <ConfirmAction actionText="结算输" disabled={!orderId || !canSettle} title="结算输" onConfirm={(reason) => settle('loss', reason)} />
      </AdminRequestActionBoundary>
    </>
  );
}

export function CreateSecondsPairAction({ onCreated }: CreateActionProps = {}) {
  const [secondsProduct, setSecondsProduct] = useState(newSecondsProduct);
  const [activeTab, setActiveTab] = useState<SecondsProductTab>('basic');
  const { assetLoading, assetOptions } = useAssetOptions();
  const { pairLoading, pairOptions } = useMarketPairOptions();
  const updatePeriod = (rowId: string, patch: Partial<SecondsProductPeriodValues>) => {
    setSecondsProduct((current) => ({
      ...current,
      periods: current.periods.map((period) => (period.rowId === rowId ? { ...period, ...patch } : period))
    }));
  };
  const addPeriod = () => {
    setSecondsProduct((current) => ({
      ...current,
      periods: [...current.periods, newSecondsProductPeriod()]
    }));
  };
  const removePeriod = (rowId: string) => {
    setSecondsProduct((current) => ({
      ...current,
      periods: current.periods.length > 1 ? current.periods.filter((period) => period.rowId !== rowId) : current.periods
    }));
  };

  return (
    <FormModal actionText="添加秒合约交易对" size="wide" title="添加秒合约交易对">
      {({ close }) => (
      <Card bordered={false}>
        <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
          <SecondsProductTabs activeTab={activeTab} onActiveTabChange={setActiveTab}>
            {activeTab === 'basic' ? (
            <div className="admin-action-form admin-action-form-wide">
              <MarketPairSelect
                label="秒合约交易对"
                loading={pairLoading}
                options={pairOptions}
                value={secondsProduct.pairId}
                onChange={(pairId) => setSecondsProduct({ ...secondsProduct, pairId })}
              />
              <AssetSelect
                label="押注资产"
                loading={assetLoading}
                options={assetOptions}
                value={secondsProduct.stakeAsset}
                onChange={(stakeAsset) => setSecondsProduct({ ...secondsProduct, stakeAsset })}
              />
              <AdminImageUpload label="秒合约交易对 Logo" value={secondsProduct.logoUrl} variant="avatar" onChange={(logoUrl) => setSecondsProduct({ ...secondsProduct, logoUrl })} />
              <label>
                初始状态
                <AdminSelect ariaLabel="初始状态" onChange={(status) => setSecondsProduct({ ...secondsProduct, status })} optionList={statusOptions} value={secondsProduct.status} />
              </label>
            </div>
            ) : (
            <SecondsProductPeriodsEditor periods={secondsProduct.periods} onAdd={addPeriod} onRemove={removePeriod} onUpdate={updatePeriod} />
            )}
          </SecondsProductTabs>
          <div className="admin-action-footer">
            <ConfirmAction
              actionText="提交添加秒合约交易对"
              disabled={!isSecondsProductCreatable(secondsProduct)}
              title="确认添加秒合约交易对"
              onConfirm={async (reason) => {
                await submitAction('添加秒合约交易对', () =>
                  apiRequest('/admin/api/v1/seconds-contracts/products', {
                    method: 'POST',
                    body: JSON.stringify(secondsProductRequestBody(secondsProduct, reason))
                  })
                );
                completeCreate(close, onCreated, () => {
                  setSecondsProduct(newSecondsProduct());
                  setActiveTab('basic');
                });
              }}
            />
          </div>
        </Space>
      </Card>
      )}
    </FormModal>
  );
}
