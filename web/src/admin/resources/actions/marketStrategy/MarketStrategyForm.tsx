import { Button, Toast } from '@douyinfe/semi-ui';

import {
  AdminCheckbox,
  AdminSelect,
  AdminTextInput
} from '../../../../shared/SemiFormControls';
import { MarketStrategyNodeEditor } from '../../../components/MarketStrategyNodeEditor';
import { MarketPairSelect, useMarketPairOptions } from '../shared';
import {
  applyPreset,
  eligibleMarketStrategyPairs,
  isMarketStrategySubmittable,
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
  const presets = useMarketStrategyPresets(active);
  const { pairLoading, pairOptions } = useMarketPairOptions(active && includePairId);
  const selectedPreset = presets.presets.find((preset) => preset.code === values.scenario);
  const canPreview = isMarketStrategySubmittable(values, true);
  const validationError = marketStrategyValidationError(values, true);
  const selectablePairs = eligibleMarketStrategyPairs(pairOptions);
  const selectableStrategyTypes = strategyTypeOptionsWithCurrent(values.strategyType);

  return (
    <div className="admin-market-strategy-form">
      <section className="admin-market-strategy-section">
        <div className="admin-market-strategy-section__heading">
          <div><h3>策略基础配置</h3><p>定义权威 1m 行情的交易对、时间范围、起止价格和全局量价边界。</p></div>
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
          <label>波动率<AdminTextInput ariaLabel="波动率" value={values.volatility} onChange={(volatility) => onChange({ ...values, volatility })} /></label>
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
            <label>当前状态<AdminTextInput ariaLabel="当前状态" readOnly value={values.status} onChange={() => undefined} /></label>
          )}
        </div>
      </section>

      <section className="admin-market-strategy-section admin-market-generator-section">
        <div className="admin-market-strategy-section__heading">
          <div><h3>生成模型与场景</h3><p>场景只填充显式节点和参数；最终版本不依赖隐藏规则，可审计、可重放。</p></div>
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
              onChange(next);
              Toast.success(`已应用“${selectedPreset.name}”预设，所有参数仍可继续修改`);
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
        <div className="admin-action-form admin-market-generator-fields">
          <label>
            Seed 模式
            <AdminSelect
              ariaLabel="Seed 模式"
              onChange={(seedMode) => onChange({ ...values, seedMode, regenerateSeed: false })}
              optionList={seedModeOptions}
              value={values.seedMode}
            />
          </label>
          {values.seedMode === 'fixed' ? (
            <label>固定 Seed<AdminTextInput ariaLabel="固定 Seed" placeholder="1～128 个字符" value={values.seed} onChange={(seed) => onChange({ ...values, seed })} /></label>
          ) : (
            <label>
              当前实际 Seed
              <AdminTextInput ariaLabel="当前实际 Seed" placeholder={isEditing ? '读取当前激活版本' : '创建时由后端生成'} readOnly value={values.seed} onChange={() => undefined} />
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
              为本次新版本重新生成 Seed；未选中时继承当前激活版本，保持随机纹理连续
            </AdminCheckbox>
          </div>
        ) : null}
      </section>

      <MarketStrategyNodeEditor value={values.nodes} onChange={(nodes) => onChange({ ...values, nodes })} />
      {validationError ? <div aria-live="polite" className="admin-inline-error" role="alert">{validationError}</div> : null}
    </div>
  );
}
