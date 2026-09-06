import { Button, Modal, Toast } from '@douyinfe/semi-ui';
import { useState } from 'react';
import { adminEnumLabel } from '../../../../shared/adminPresentation';

import {
  AdminCheckbox,
  AdminSelect,
  AdminTextInput
} from '../../../../shared/SemiFormControls';
import { MarketStrategyNodeEditor } from '../../../components/MarketStrategyNodeEditor';
import { MarketStrategyVolatilityField } from '../../../components/MarketStrategyVolatilityField';
import { MarketPairSelect, useMarketPairOptions } from '../shared';
import {
  applyPreset,
  eligibleMarketStrategyPairs,
  isMarketStrategySubmittable,
  marketStrategyShapeWarnings,
  marketStrategyValidationError,
  scenarioOptions,
  seedModeOptions,
  strategyTypeOptionsWithCurrent,
  volumeShapeOptions
} from './model';
import { MarketStrategyPreviewAction } from './MarketStrategyPreviewAction';
import type { MarketStrategyValues } from './types';
import { useMarketStrategyPresets } from './useMarketStrategyPresets';

export function MarketStrategyForm({
  active,
  includePairId,
  isEditing,
  onChange,
  strategyId,
  values
}: {
  active: boolean;
  includePairId: boolean;
  isEditing: boolean;
  onChange: (values: MarketStrategyValues) => void;
  strategyId?: string;
  values: MarketStrategyValues;
}) {
  const [pendingPreset, setPendingPreset] = useState<{ name: string; values: MarketStrategyValues } | null>(null);
  const presets = useMarketStrategyPresets(active);
  const { pairLoading, pairOptions } = useMarketPairOptions(active && includePairId);
  const selectedPreset = presets.presets.find((preset) => preset.code === values.scenario);
  const canPreview = isMarketStrategySubmittable(values, true);
  const validationError = marketStrategyValidationError(values, true);
  const selectablePairs = eligibleMarketStrategyPairs(pairOptions);
  const selectableStrategyTypes = strategyTypeOptionsWithCurrent(values.strategyType);
  const shapeWarnings = marketStrategyShapeWarnings(values);

  return (
    <div className="admin-market-strategy-form">
      <section className="admin-market-strategy-section">
        <div className="admin-market-strategy-section__heading">
          <div><h3>策略基础配置</h3><p>定义权威 1m 行情的交易对、时间范围、起止价格和全局量价边界。订单簿、逐笔成交和高周期由同一行情自动派生。</p></div>
        </div>
        <div className="admin-action-form">
          {includePairId ? (
            <MarketPairSelect
              label="交易对ID"
              loading={pairLoading}
              onChange={(pairId) => onChange({ ...values, pairId })}
              options={selectablePairs}
              value={values.pairId}
            />
          ) : null}
          {!includePairId ? (
            <label>交易对ID<AdminTextInput ariaLabel="交易对ID" readOnly value={values.pairId} onChange={() => undefined} /></label>
          ) : null}
          <label>
            策略类型
            <AdminSelect
              ariaLabel="策略类型"
              onChange={(strategyType) => onChange({ ...values, strategyType })}
              optionList={selectableStrategyTypes}
              value={values.strategyType}
            />
          </label>
          <label>起始价<AdminTextInput ariaLabel="起始价" value={values.startPrice} onChange={(startPrice) => onChange({ ...values, startPrice })} /></label>
          <label>目标价<AdminTextInput ariaLabel="目标价" value={values.targetPrice} onChange={(targetPrice) => onChange({ ...values, targetPrice })} /></label>
          <label>开始时间<AdminTextInput ariaLabel="开始时间" type="datetime-local" value={values.startTime} onChange={(startTime) => onChange({ ...values, startTime })} /></label>
          <label>结束时间<AdminTextInput ariaLabel="结束时间" type="datetime-local" value={values.endTime} onChange={(endTime) => onChange({ ...values, endTime })} /></label>
          <MarketStrategyVolatilityField value={values.volatility} onChange={(volatility) => onChange({ ...values, volatility })} />
          <label>最小成交量<AdminTextInput ariaLabel="最小成交量" value={values.volumeMin} onChange={(volumeMin) => onChange({ ...values, volumeMin })} /></label>
          <label>最大成交量<AdminTextInput ariaLabel="最大成交量" value={values.volumeMax} onChange={(volumeMax) => onChange({ ...values, volumeMax })} /></label>
          {includePairId ? (
            <label>
              初始状态
              <AdminSelect
                ariaLabel="初始状态"
                onChange={(status) => onChange({ ...values, status })}
                optionList={[
                  { value: 'draft', label: '草稿' },
                  { value: 'active', label: '启用' },
                  { value: 'paused', label: '暂停' },
                  { value: 'disabled', label: '禁用' }
                ]}
                value={values.status}
              />
            </label>
          ) : (
            <label>当前状态<AdminTextInput ariaLabel="当前状态" readOnly value={adminEnumLabel('status', values.status) ?? values.status} onChange={() => undefined} /></label>
          )}
        </div>
      </section>

      <p>成交量范围按每分钟累计数量配置，模拟逐笔按秒分摊；成交量为 0 时不生成虚假成交。模拟盘口与逐笔仅用于行情展示，不是用户真实成交。</p>

      <section className="admin-market-strategy-section admin-market-generator-section">
        <div className="admin-market-strategy-section__heading">
          <div><h3>生成模型与场景</h3><p>切换场景只改变场景标记。应用预设需另行确认，只替换生成参数和路径节点，保留起止价格、时间、随机种子及全局量价边界；节点可能与手动目标方向不同，请预览确认。</p></div>
          <MarketStrategyPreviewAction disabled={!canPreview} strategyId={strategyId} values={values} />
        </div>
        <div className="admin-market-preset-bar">
          <label>
            行情场景
            <AdminSelect
              ariaLabel="行情场景"
              loading={presets.loading}
              onChange={(scenario) => onChange({ ...values, scenario })}
              optionList={scenarioOptions}
              value={values.scenario}
            />
          </label>
          <Button
            disabled={!selectedPreset || presets.loading}
            onClick={() => {
              if (!selectedPreset) return;
              const next = applyPreset(values, selectedPreset);
              if (!next) {
                Toast.warning('请填写有效起始价和整分钟起止时间，并扩大时间范围以容纳全部预设节点；当前配置未更改');
                return;
              }
              setPendingPreset({ name: selectedPreset.name, values: next });
            }}
            theme="solid"
            type="primary"
          >
            应用场景预设
          </Button>
          <div className="admin-market-preset-description">
            {presets.error ? (
              <span role="alert">
                预设加载失败：{presets.error}
                <Button onClick={presets.reload} size="small" theme="borderless">重新加载</Button>
              </span>
            ) : selectedPreset?.description ?? '选择场景后可一键生成显式参数与时间节点。'}
          </div>
        </div>
        <Modal
          visible={pendingPreset !== null}
          title="确认应用场景预设"
          maskClosable={false}
          motion={false}
          cancelText="保留当前配置"
          okText="替换节点与生成参数"
          cancelButtonProps={{ 'aria-label': '保留当前配置' }}
          okButtonProps={{ 'aria-label': '替换节点与生成参数' }}
          onCancel={() => setPendingPreset(null)}
          onOk={() => {
            if (!pendingPreset) return;
            onChange(pendingPreset.values);
            Toast.success(`已应用“${pendingPreset.name}”预设，手动价格和随机种子保持不变`);
            setPendingPreset(null);
          }}
        >
          <p>将现有 {values.nodes.length} 个节点替换为“{pendingPreset?.name}”的 {pendingPreset?.values.nodes.length ?? 0} 个节点，并替换均值回归、噪声、影线和成交量形态。</p>
          <p>起始价 {values.startPrice || '未填写'}、目标价 {values.targetPrice || '未填写'}、起止时间、随机种子和全局波动率/成交量范围均保持不变。本操作只修改草稿，不保存或启用策略。</p>
        </Modal>
        <div className="admin-action-form admin-market-generator-fields">
          <label>
            随机种子模式
            <AdminSelect
              ariaLabel="随机种子模式"
              onChange={(seedMode) => onChange({ ...values, seedMode, regenerateSeed: false })}
              optionList={seedModeOptions}
              value={values.seedMode}
            />
          </label>
          {values.seedMode === 'fixed' ? (
            <label>固定随机种子<AdminTextInput ariaLabel="固定随机种子" placeholder="1～128 个字符" value={values.seed} onChange={(seed) => onChange({ ...values, seed })} /></label>
          ) : (
            <label>
              当前实际随机种子
              <AdminTextInput ariaLabel="当前实际随机种子" placeholder={isEditing ? '读取当前配置版本' : '创建时由后端生成'} readOnly value={values.seed} onChange={() => undefined} />
            </label>
          )}
          <label>均值回归强度（0～2）<AdminTextInput ariaLabel="均值回归强度" value={values.meanReversionStrength} onChange={(meanReversionStrength) => onChange({ ...values, meanReversionStrength })} /></label>
          <label>噪声强度（0～5）<AdminTextInput ariaLabel="噪声强度" value={values.noiseScale} onChange={(noiseScale) => onChange({ ...values, noiseScale })} /></label>
          <label>影线强度（0～5）<AdminTextInput ariaLabel="影线强度" value={values.wickScale} onChange={(wickScale) => onChange({ ...values, wickScale })} /></label>
          <label>
            成交量形态
            <AdminSelect ariaLabel="成交量形态" onChange={(volumeShape) => onChange({ ...values, volumeShape })} optionList={volumeShapeOptions} value={values.volumeShape} />
          </label>
        </div>
        {isEditing && values.seedMode === 'auto' ? (
          <div className="admin-market-seed-command">
            <AdminCheckbox checked={values.regenerateSeed} onChange={(regenerateSeed) => onChange({ ...values, regenerateSeed })}>
              为本次新版本重新生成随机种子；未选中时继承当前配置版本，保持随机纹理连续
            </AdminCheckbox>
          </div>
        ) : null}
      </section>

      <MarketStrategyNodeEditor value={values.nodes} onChange={(nodes) => onChange({ ...values, nodes })} />
      <p className="admin-market-volatility-hint">影线按「实体端点价格 × 当前波动率 × 随机比例 × 影线强度」生成，与收盘价噪声独立；请同时检查高低价，而非只看收盘走势。</p>
      {shapeWarnings.length ? (
        <div aria-label="影线参数提示" className="admin-market-shape-warning" role="note">
          {shapeWarnings.map((warning) => <p key={warning}>{warning}</p>)}
        </div>
      ) : null}
      {validationError ? <div aria-live="polite" className="admin-inline-error" role="alert">{validationError}</div> : null}
    </div>
  );
}
