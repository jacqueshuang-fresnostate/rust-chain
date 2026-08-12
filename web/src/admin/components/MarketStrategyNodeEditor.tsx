import { Button, Card } from '@douyinfe/semi-ui';

import { AdminSelect, AdminTextInput, type SemiSelectOption } from '../../shared/SemiFormControls';

export type MarketStrategyNodeDraft = {
  clientId: string;
  executionMode: 'hard' | 'soft' | 'range';
  targetTime: string;
  targetType: 'absolute_price' | 'percent_from_start' | 'percent_from_previous';
  targetValue: string;
  tolerance: string;
  volatility: string;
  volumeMax: string;
  volumeMin: string;
};

const targetTypeOptions: SemiSelectOption[] = [
  { value: 'absolute_price', label: '绝对价格' },
  { value: 'percent_from_start', label: '相对起始价百分比' },
  { value: 'percent_from_previous', label: '相对上一节点百分比' }
];

const executionModeOptions: SemiSelectOption[] = [
  { value: 'hard', label: '硬命中' },
  { value: 'soft', label: '软命中' },
  { value: 'range', label: '范围命中' }
];

let nextNodeId = 0;

export function createMarketStrategyNodeDraft(): MarketStrategyNodeDraft {
  nextNodeId += 1;
  return {
    clientId: `strategy-node-${nextNodeId}`,
    targetTime: '',
    targetType: 'absolute_price',
    targetValue: '',
    executionMode: 'hard',
    tolerance: '0',
    volatility: '0',
    volumeMin: '',
    volumeMax: ''
  };
}

type MarketStrategyNodeEditorProps = {
  disabled?: boolean;
  onChange: (nodes: MarketStrategyNodeDraft[]) => void;
  value: MarketStrategyNodeDraft[];
};

export function MarketStrategyNodeEditor({ disabled = false, onChange, value }: MarketStrategyNodeEditorProps) {
  function updateNode(index: number, patch: Partial<MarketStrategyNodeDraft>) {
    onChange(value.map((node, nodeIndex) => (nodeIndex === index ? { ...node, ...patch } : node)));
  }

  return (
    <section aria-labelledby="market-strategy-node-editor-title" className="admin-market-strategy-node-editor">
      <div className="admin-market-strategy-node-editor-heading">
        <div>
          <h3 id="market-strategy-node-editor-title">策略目标节点</h3>
          <p>节点按时间顺序执行；时间需位于策略区间内并对齐到整分钟。</p>
        </div>
        <Button
          aria-label="新增策略节点"
          disabled={disabled}
          onClick={() => onChange([...value, createMarketStrategyNodeDraft()])}
          size="small"
          theme="solid"
          type="primary"
        >
          新增节点
        </Button>
      </div>

      {value.length === 0 ? (
        <div aria-live="polite" className="admin-market-strategy-node-empty">暂无目标节点，将继续使用兼容终点。</div>
      ) : (
        <div className="admin-market-strategy-node-list">
          {value.map((node, index) => {
            const rowName = `节点${index + 1}`;
            return (
              <Card bordered className="admin-market-strategy-node-card" key={node.clientId}>
                <div className="admin-market-strategy-node-card-heading">
                  <strong>{rowName}</strong>
                  <Button
                    aria-label={`删除${rowName}`}
                    disabled={disabled}
                    onClick={() => onChange(value.filter((_, nodeIndex) => nodeIndex !== index))}
                    size="small"
                    theme="borderless"
                    type="danger"
                  >
                    删除
                  </Button>
                </div>
                <div className="admin-action-form admin-market-strategy-node-fields">
                  <label>
                    目标时间
                    <AdminTextInput
                      ariaLabel={`${rowName}目标时间`}
                      disabled={disabled}
                      onChange={(targetTime) => updateNode(index, { targetTime })}
                      type="datetime-local"
                      value={node.targetTime}
                    />
                  </label>
                  <label>
                    目标类型
                    <AdminSelect
                      ariaLabel={`${rowName}目标类型`}
                      disabled={disabled}
                      onChange={(targetType) => updateNode(index, { targetType: targetType as MarketStrategyNodeDraft['targetType'] })}
                      optionList={targetTypeOptions}
                      value={node.targetType}
                    />
                  </label>
                  <label>
                    目标值
                    <AdminTextInput ariaLabel={`${rowName}目标值`} disabled={disabled} onChange={(targetValue) => updateNode(index, { targetValue })} value={node.targetValue} />
                  </label>
                  <label>
                    执行模式
                    <AdminSelect
                      ariaLabel={`${rowName}执行模式`}
                      disabled={disabled}
                      onChange={(executionMode) => updateNode(index, { executionMode: executionMode as MarketStrategyNodeDraft['executionMode'] })}
                      optionList={executionModeOptions}
                      value={node.executionMode}
                    />
                  </label>
                  <label>
                    容差
                    <AdminTextInput ariaLabel={`${rowName}容差`} disabled={disabled} onChange={(tolerance) => updateNode(index, { tolerance })} value={node.tolerance} />
                  </label>
                  <label>
                    局部波动率
                    <AdminTextInput ariaLabel={`${rowName}局部波动率`} disabled={disabled} onChange={(volatility) => updateNode(index, { volatility })} value={node.volatility} />
                  </label>
                  <label>
                    最小成交量（可选）
                    <AdminTextInput ariaLabel={`${rowName}最小成交量`} disabled={disabled} onChange={(volumeMin) => updateNode(index, { volumeMin })} value={node.volumeMin} />
                  </label>
                  <label>
                    最大成交量（可选）
                    <AdminTextInput ariaLabel={`${rowName}最大成交量`} disabled={disabled} onChange={(volumeMax) => updateNode(index, { volumeMax })} value={node.volumeMax} />
                  </label>
                </div>
              </Card>
            );
          })}
        </div>
      )}
    </section>
  );
}
