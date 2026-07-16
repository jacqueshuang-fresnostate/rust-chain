import { IconRefresh, IconSetting, IconShield } from '@douyinfe/semi-icons';
import { Button, Card, Descriptions, Divider, Image, Select, SideSheet, Space, Table, Tabs, Toast, Typography } from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import { useEffect, useMemo, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminSelect, AdminTextArea, AdminTextInput, type SemiSelectOption } from '../../shared/SemiFormControls';
import { StatusTag } from '../../shared/StatusTag';
import { TimestampText } from '../../shared/TimestampText';
import { containedTableScroll, containedTableStyle } from '../../shared/tableLayout';

const { Text, Title } = Typography;

type KycStatus = 'pending' | 'approved' | 'rejected';
type KycTab = 'config' | 'reviews';

type KycCountryDocumentTypeRule = {
  country: string;
  document_types: string[];
  handheld_document_types?: string[];
};

type AdminCountryOption = {
  country_code: string;
  country_name: string;
  registration_enabled?: boolean;
  status?: string;
};

type AdminCountriesResponse = {
  countries?: AdminCountryOption[];
};

type KycConfig = {
  allowed_countries: string[];
  country_document_types: KycCountryDocumentTypeRule[];
  enabled: boolean;
  id: number;
  max_document_size_bytes: number;
  name: string;
  required_documents: string[];
  target_kyc_level: number;
  updated_at: number;
  updated_by?: number | null;
};

type KycSubmissionSummary = Record<string, unknown> & {
  country: string;
  document_type: string;
  email?: string | null;
  id: number;
  submission_type: 'personal' | 'enterprise';
  id_number: string;
  enterprise_name?: string | null;
  business_registration_number?: string | null;
  real_name: string;
  status: KycStatus;
  submitted_at: number;
  target_kyc_level: number;
  user_id: number;
};

type KycSubmission = KycSubmissionSummary & {
  document_back_image: string;
  document_front_image: string;
  document_handheld_image?: string | null;
  document_type: string;
  phone?: string | null;
  review_reason?: string | null;
  reviewed_at?: number | null;
  reviewed_by?: number | null;
  updated_at: number;
};

type KycSubmissionsResponse = {
  submissions: KycSubmissionSummary[];
};

type ConfigForm = {
  allowedCountries: string;
  countryDocumentTypes: KycCountryDocumentTypeRule[];
  enabled: boolean;
  maxDocumentSizeMb: string;
  requiredDocuments: string[];
  targetKycLevel: string;
};

const defaultConfigForm: ConfigForm = {
  allowedCountries: '',
  countryDocumentTypes: [],
  enabled: true,
  maxDocumentSizeMb: '5',
  requiredDocuments: ['identity_front', 'identity_back'],
  targetKycLevel: '1'
};

const reviewStatusOptions: SemiSelectOption[] = [
  { value: 'pending', label: '待审核' },
  { value: 'approved', label: '已通过' },
  { value: 'rejected', label: '已拒绝' }
];

const enabledOptions: SemiSelectOption[] = [
  { value: 'enabled', label: '启用' },
  { value: 'disabled', label: '禁用' }
];

const documentOptions = [
  { value: 'identity_front', label: '身份证件正面' },
  { value: 'identity_back', label: '身份证件反面' }
];
const baseRequiredDocuments = documentOptions.map((option) => option.value);

const kycDocumentTypeOptions: SemiSelectOption[] = [
  { value: 'identity_card', label: '身份证' },
  { value: 'passport', label: '护照' },
  { value: 'driver_license', label: '驾驶证' },
  { value: 'residence_permit', label: '居住证' }
];

const kycTabs = [
  { itemKey: 'config', tab: 'KYC 配置', icon: <IconSetting aria-hidden="true" /> },
  { itemKey: 'reviews', tab: '人工审核', icon: <IconShield aria-hidden="true" /> }
];

const submissionTypeOptions = [
  { value: 'personal', label: '个人认证' },
  { value: 'enterprise', label: '企业认证' }
];

const submissionTypeLabelMap = {
  personal: '个人认证',
  enterprise: '企业认证'
} as const;

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

function splitCsv(value: string) {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function uniqueItems(items: string[]) {
  return items.filter((item, index) => item.length > 0 && items.indexOf(item) === index);
}

function configToForm(config: KycConfig): ConfigForm {
  return {
    allowedCountries: config.allowed_countries.join(','),
    countryDocumentTypes: normalizeCountryDocumentTypes(config.country_document_types),
    enabled: config.enabled,
    maxDocumentSizeMb: formatMb(config.max_document_size_bytes),
    requiredDocuments: normalizeRequiredDocuments(config.required_documents),
    targetKycLevel: String(config.target_kyc_level)
  };
}

function formatMb(bytes: number) {
  const mb = bytes / 1024 / 1024;
  return Number.isInteger(mb) ? String(mb) : mb.toFixed(2);
}

function mbToBytes(value: string) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error('单个证件大小必须大于 0 MB');
  }
  return Math.round(parsed * 1024 * 1024);
}

function positiveInteger(value: string, label: string) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label}必须为正整数`);
  }
  return parsed;
}

function maskIdentityNumber(value: string) {
  if (value.length <= 8) {
    return '*'.repeat(value.length);
  }
  return `${value.slice(0, 4)}****${value.slice(-4)}`;
}

function documentTypeLabel(value: string) {
  return kycDocumentTypeOptions.find((option) => option.value === value)?.label ?? value;
}

function normalizeRequiredDocuments(documents: string[] | undefined) {
  const supportedDocuments = new Set(baseRequiredDocuments);
  const requestedDocuments = uniqueItems(documents ?? []).filter((document) => supportedDocuments.has(document));
  return uniqueItems([...baseRequiredDocuments, ...requestedDocuments]);
}

  function normalizeCountryDocumentTypes(rules: KycCountryDocumentTypeRule[] | undefined) {
  return (rules ?? []).map((rule) => ({
    country: rule.country,
    document_types: uniqueItems(rule.document_types),
    handheld_document_types: uniqueItems(rule.handheld_document_types ?? []).filter((documentType) => rule.document_types.includes(documentType))
  }));
}

function selectValues(value: unknown) {
  if (Array.isArray(value)) {
    return uniqueItems(value.map((item) => String(item)));
  }
  if (value === undefined || value === null || value === '') {
    return [];
  }
  return [String(value)];
}

function serializeCountryDocumentTypes(rules: KycCountryDocumentTypeRule[]) {
  return rules
    .map((rule) => {
      const documentTypes = uniqueItems(rule.document_types);
      return {
        country: rule.country.trim(),
        document_types: documentTypes,
        handheld_document_types: uniqueItems(rule.handheld_document_types ?? []).filter((documentType) => documentTypes.includes(documentType))
      };
    })
    .filter((rule) => rule.country && rule.document_types.length > 0);
}

function adminCountrySelectOptions(countries: AdminCountryOption[]): SemiSelectOption[] {
  return countries.map((country) => ({
    value: country.country_name,
    label: `${country.country_name} (${country.country_code})`
  }));
}

function includeLegacyCountryOptions(options: SemiSelectOption[], rules: KycCountryDocumentTypeRule[], allowedCountries: string) {
  const optionValues = new Set(options.map((option) => option.value.toLowerCase()));
  const legacyValues = uniqueItems([...splitCsv(allowedCountries), ...rules.map((rule) => rule.country)])
    .filter((country) => !optionValues.has(country.toLowerCase()))
    .map((country) => ({ value: country, label: country }));
  return [...options, ...legacyValues];
}

function ruleDocumentTypeOptions(rule: KycCountryDocumentTypeRule): SemiSelectOption[] {
  const allowedTypes = rule.document_types.length > 0 ? rule.document_types : kycDocumentTypeOptions.map((option) => option.value);
  return kycDocumentTypeOptions.filter((option) => allowedTypes.includes(option.value));
}

function countHandheldDocumentTypeRules(rules: KycCountryDocumentTypeRule[]) {
  return rules.reduce((count, rule) => count + uniqueItems(rule.handheld_document_types ?? []).length, 0);
}

async function submitAction(label: string, request: () => Promise<unknown>) {
  try {
    await request();
    Toast.success(`${label}已提交`);
  } catch (error) {
    Toast.error(errorMessage(error));
    throw error;
  }
}

function documentLabel(value: string) {
  return documentOptions.find((option) => option.value === value)?.label ?? value;
}

export function KycManagementPage() {
  const [activeTab, setActiveTab] = useState<KycTab>('reviews');
  const [config, setConfig] = useState<KycConfig | null>(null);
  const [configForm, setConfigForm] = useState<ConfigForm>(defaultConfigForm);
  const [countryOptions, setCountryOptions] = useState<SemiSelectOption[]>([]);
  const [detail, setDetail] = useState<KycSubmission | null>(null);
  const [loading, setLoading] = useState(true);
  const [reviewReason, setReviewReason] = useState('');
  const [reviewStatus, setReviewStatus] = useState<KycStatus>('pending');
  const [reviewing, setReviewing] = useState<KycStatus | null>(null);
  const [reviewLevel, setReviewLevel] = useState('1');
  const [submissions, setSubmissions] = useState<KycSubmissionSummary[]>([]);
  const handheldDocumentTypeRuleCount = countHandheldDocumentTypeRules(configForm.countryDocumentTypes);

  const configSummary = useMemo(
    () => [
      `状态：${config?.enabled ? '启用' : '禁用'}`,
      `目标等级：${config?.target_kyc_level ?? '-'}`,
      `证件大小：${config ? formatMb(config.max_document_size_bytes) : '-'} MB`,
      `基础必传：${normalizeRequiredDocuments(config?.required_documents).map(documentLabel).join('，')}`,
      `证件类型规则：${config?.country_document_types.length || 0} 个国家`,
      `手持照规则：${countHandheldDocumentTypeRules(config?.country_document_types ?? [])} 个证件类型`
    ],
    [config]
  );

  const countryRuleOptions = useMemo(
    () => includeLegacyCountryOptions(countryOptions, configForm.countryDocumentTypes, configForm.allowedCountries),
    [configForm.allowedCountries, configForm.countryDocumentTypes, countryOptions]
  );

  async function loadPage(nextStatus = reviewStatus) {
    setLoading(true);
    try {
      const [nextConfig, submissionResponse, countriesResponse] = await Promise.all([
        apiRequest<KycConfig>('/admin/api/v1/kyc/config'),
        apiRequest<KycSubmissionsResponse>(`/admin/api/v1/kyc/submissions?status=${nextStatus}&limit=100`),
        apiRequest<AdminCountriesResponse>('/admin/api/v1/countries?status=active&limit=200')
      ]);
      setConfig(nextConfig);
      setConfigForm(configToForm(nextConfig));
      setSubmissions(submissionResponse.submissions);
      setCountryOptions(adminCountrySelectOptions(countriesResponse.countries ?? []));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadPage().catch((error) => Toast.error(errorMessage(error)));
  }, []);

  async function openDetail(submissionId: number) {
    try {
      const nextDetail = await apiRequest<KycSubmission>(`/admin/api/v1/kyc/submissions/${submissionId}`);
      setDetail(nextDetail);
      setReviewLevel(String(nextDetail.target_kyc_level));
      setReviewReason('');
    } catch (error) {
      Toast.error(errorMessage(error));
      throw error;
    }
  }

  async function reviewSubmission(status: Exclude<KycStatus, 'pending'>) {
    if (!detail) {
      return;
    }
    const reason = reviewReason.trim();
    if (!reason) {
      Toast.error('请输入审核原因');
      return;
    }
    setReviewing(status);
    try {
      await submitAction(status === 'approved' ? '审核通过' : '审核拒绝', async () => {
        await apiRequest(`/admin/api/v1/kyc/submissions/${detail.id}/review`, {
          method: 'PATCH',
          body: JSON.stringify({
            kyc_level: status === 'approved' ? positiveInteger(reviewLevel, 'KYC 等级') : undefined,
            reason,
            status
          })
        });
      });
      setDetail(null);
      await loadPage(reviewStatus);
    } finally {
      setReviewing(null);
    }
  }

  function updateCountryDocumentRule(index: number, nextRule: Partial<KycCountryDocumentTypeRule>) {
    setConfigForm((current) => ({
      ...current,
      countryDocumentTypes: current.countryDocumentTypes.map((rule, ruleIndex) => {
        if (ruleIndex !== index) {
          return rule;
        }
        const updatedRule = { ...rule, ...nextRule };
        if (nextRule.document_types) {
          updatedRule.handheld_document_types = (updatedRule.handheld_document_types ?? []).filter((documentType) => nextRule.document_types?.includes(documentType));
        }
        return updatedRule;
      })
    }));
  }

  function addCountryDocumentRule() {
    setConfigForm((current) => ({
      ...current,
      countryDocumentTypes: [...current.countryDocumentTypes, { country: '', document_types: ['identity_card'], handheld_document_types: [] }]
    }));
  }

  function removeCountryDocumentRule(index: number) {
    setConfigForm((current) => ({
      ...current,
      countryDocumentTypes: current.countryDocumentTypes.filter((_, ruleIndex) => ruleIndex !== index)
    }));
  }

  const columns = useMemo<Array<ColumnProps<KycSubmissionSummary>>>(
    () => [
      { dataIndex: 'id', key: 'id', title: '申请ID', width: 110 },
      { dataIndex: 'user_id', key: 'user_id', title: '用户ID', width: 110 },
      { dataIndex: 'email', key: 'email', title: '邮箱', width: 220, render: (value) => <span>{typeof value === 'string' && value ? value : '-'}</span> },
      {
        dataIndex: 'submission_type',
        key: 'submission_type',
        title: '认证类型',
        width: 120,
        render: (value) => <span>{typeof value === 'string' ? submissionTypeLabelMap[value as keyof typeof submissionTypeLabelMap] ?? value : '-'}</span>
      },
      { dataIndex: 'real_name', key: 'real_name', title: '姓名', width: 160 },
      {
        dataIndex: 'enterprise_name',
        key: 'enterprise_name',
        title: '企业名称',
        width: 180,
        render: (value) => <span>{typeof value === 'string' && value ? value : '-'}</span>
      },
      {
        dataIndex: 'business_registration_number',
        key: 'business_registration_number',
        title: '统一社会信用代码',
        width: 220,
        render: (value) => <span>{typeof value === 'string' && value ? value : '-'}</span>
      },
      { dataIndex: 'country', key: 'country', title: '国家', width: 140 },
      { dataIndex: 'document_type', key: 'document_type', title: '证件类型', width: 130, render: (value) => <span>{typeof value === 'string' ? documentTypeLabel(value) : '-'}</span> },
      { dataIndex: 'id_number', key: 'id_number', title: '证件号', width: 180, render: (value) => <span>{typeof value === 'string' ? maskIdentityNumber(value) : '-'}</span> },
      { dataIndex: 'status', key: 'status', title: '状态', width: 120, render: (value) => <StatusTag value={typeof value === 'string' ? value : null} /> },
      { dataIndex: 'target_kyc_level', key: 'target_kyc_level', title: '目标等级', width: 110 },
      { dataIndex: 'submitted_at', key: 'submitted_at', title: '提交时间', width: 190, render: (value) => <TimestampText value={typeof value === 'number' ? value : null} /> },
      {
        dataIndex: 'id',
        key: 'actions',
        title: '操作',
        width: 120,
        render: (value) => (
          <Button disabled={typeof value !== 'number'} onClick={() => openDetail(Number(value))} size="small" theme="borderless">
            查看
          </Button>
        )
      }
    ],
    []
  );

  const countryDocumentColumns = useMemo<Array<ColumnProps<KycCountryDocumentTypeRule & { index: number }>>>(
    () => [
      {
        dataIndex: 'country',
        key: 'country',
        title: '国家 / 地区',
        width: 220,
        render: (_value, record) => (
          <div aria-label={`规则国家 ${record.index + 1}`}>
            <Select
              filter
              onChange={(country) => updateCountryDocumentRule(record.index, { country: String(country) })}
              onSelect={(country) => updateCountryDocumentRule(record.index, { country: String(country) })}
              optionList={countryRuleOptions}
              placeholder="请选择国家 / 地区"
              showClear
              style={{ width: '100%' }}
              value={record.country}
            />
          </div>
        )
      },
      {
        dataIndex: 'document_types',
        key: 'document_types',
        title: '允许证件类型',
        render: (_value, record) => (
          <Select
            aria-label={`允许证件类型 ${record.index + 1}`}
            filter
            maxTagCount={3}
            multiple
            onChange={(value) => updateCountryDocumentRule(record.index, { document_types: selectValues(value) })}
            optionList={kycDocumentTypeOptions}
            placeholder="请选择证件类型"
            showClear
            style={{ width: '100%' }}
            value={record.document_types}
          />
        )
      },
      {
        dataIndex: 'handheld_document_types',
        key: 'handheld_document_types',
        title: '需手持证件照',
        render: (_value, record) => (
          <Select
            aria-label={`需手持证件照 ${record.index + 1}`}
            disabled={record.document_types.length === 0}
            filter
            maxTagCount={3}
            multiple
            onChange={(value) => updateCountryDocumentRule(record.index, { handheld_document_types: selectValues(value) })}
            optionList={ruleDocumentTypeOptions(record)}
            placeholder="不需要可留空"
            showClear
            style={{ width: '100%' }}
            value={record.handheld_document_types ?? []}
          />
        )
      },
      {
        dataIndex: 'index',
        key: 'actions',
        title: '操作',
        width: 100,
        render: (_value, record) => (
          <Button onClick={() => removeCountryDocumentRule(record.index)} theme="borderless" type="danger">
            删除
          </Button>
        )
      }
    ],
    [countryRuleOptions]
  );

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader
        actions={
          <Button icon={<IconRefresh aria-hidden="true" />} loading={loading} onClick={() => loadPage().catch((error) => Toast.error(errorMessage(error)))} theme="borderless">
            刷新
          </Button>
        }
        title="KYC 管理"
      />
      <Card bordered={false} className="admin-action-workbench" shadows="always">
        <Tabs activeKey={activeTab} className="admin-action-tabs" onChange={(key) => setActiveTab(key as KycTab)} tabList={kycTabs} type="button" />

        {activeTab === 'config' ? (
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>KYC 配置</Title>
            <div className="admin-action-form">
              <label>
                配置状态
                <AdminSelect
                  ariaLabel="KYC 配置状态"
                  onChange={(enabled) => setConfigForm({ ...configForm, enabled: enabled === 'enabled' })}
                  optionList={enabledOptions}
                  value={configForm.enabled ? 'enabled' : 'disabled'}
                />
              </label>
              <label>
                目标 KYC 等级
                <AdminTextInput ariaLabel="目标 KYC 等级" onChange={(targetKycLevel) => setConfigForm({ ...configForm, targetKycLevel })} value={configForm.targetKycLevel} />
              </label>
              <label>
                单个证件大小 MB
                <AdminTextInput ariaLabel="单个证件大小 MB" onChange={(maxDocumentSizeMb) => setConfigForm({ ...configForm, maxDocumentSizeMb })} value={configForm.maxDocumentSizeMb} />
              </label>
              <label>
                允许国家
                <AdminTextInput ariaLabel="允许国家" onChange={(allowedCountries) => setConfigForm({ ...configForm, allowedCountries })} placeholder="留空表示不限" value={configForm.allowedCountries} />
              </label>
              <fieldset className="admin-action-choice-group">
                <legend>必传证件</legend>
                <div className="admin-action-choice-list">
                  {documentOptions.map((option) => (
                    <Text key={option.value} strong>{option.label}</Text>
                  ))}
                  <Text type="secondary">本人手持证件照：{handheldDocumentTypeRuleCount > 0 ? `${handheldDocumentTypeRuleCount} 个证件类型` : '未配置'}</Text>
                </div>
              </fieldset>
            </div>
            <Space align="center" style={{ width: '100%', justifyContent: 'space-between' }}>
              <Title heading={5}>证件类型规则</Title>
              <Button onClick={addCountryDocumentRule} theme="solid" type="primary">
                添加国家规则
              </Button>
            </Space>
            <Table
              aria-label="KYC 证件类型规则"
              bordered
              columns={countryDocumentColumns}
              dataSource={configForm.countryDocumentTypes.map((rule, index) => ({ ...rule, index }))}
              pagination={false}
              rowKey="index"
              scroll={containedTableScroll}
              style={containedTableStyle}
            />
            <div className="admin-action-summary">
              {configSummary.map((item) => (
                <span key={item}>{item}</span>
              ))}
              <span>最后更新：<TimestampText value={config?.updated_at ?? null} /></span>
            </div>
            <ConfirmAction
              actionText="保存配置"
              disabled={normalizeRequiredDocuments(configForm.requiredDocuments).length === 0}
              title="确认保存 KYC 配置"
              onConfirm={(reason) =>
                submitAction('保存 KYC 配置', async () => {
                  const saved = await apiRequest<KycConfig>('/admin/api/v1/kyc/config', {
                    method: 'PATCH',
                    body: JSON.stringify({
                      allowed_countries: uniqueItems(splitCsv(configForm.allowedCountries)),
                      country_document_types: serializeCountryDocumentTypes(configForm.countryDocumentTypes),
                      enabled: configForm.enabled,
                      max_document_size_bytes: mbToBytes(configForm.maxDocumentSizeMb),
                      reason,
                      required_documents: normalizeRequiredDocuments(configForm.requiredDocuments),
                      target_kyc_level: positiveInteger(configForm.targetKycLevel, '目标 KYC 等级')
                    })
                  });
                  setConfig(saved);
                  setConfigForm(configToForm(saved));
                })
              }
            />
          </Space>
        ) : null}

        {activeTab === 'reviews' ? (
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>人工审核</Title>
            <div className="admin-action-form admin-action-form-narrow">
              <label>
                审核状态
                <AdminSelect
                  ariaLabel="审核状态"
                  onChange={(status) => {
                    const nextStatus = status as KycStatus;
                    setReviewStatus(nextStatus);
                    loadPage(nextStatus).catch((error) => Toast.error(errorMessage(error)));
                  }}
                  optionList={reviewStatusOptions}
                  value={reviewStatus}
                />
              </label>
            </div>
            <Table
              aria-label="KYC 审核列表"
              bordered
              columns={columns}
              dataSource={submissions}
              loading={loading}
              pagination={{ pageSize: 20, showSizeChanger: true }}
              resizable
              rowKey="id"
              scroll={containedTableScroll}
              style={containedTableStyle}
            />
          </Space>
        ) : null}
      </Card>

      <SideSheet onCancel={() => setDetail(null)} title="KYC 审核详情" visible={detail !== null} width={760}>
        {detail ? (
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Descriptions
              column={2}
              data={[
                { key: '申请ID', value: detail.id },
                { key: '用户ID', value: detail.user_id },
                { key: '邮箱', value: detail.email || '-' },
                { key: '手机号', value: detail.phone || '-' },
                { key: '姓名', value: detail.real_name },
                { key: '认证类型', value: submissionTypeLabelMap[detail.submission_type] || detail.submission_type },
                { key: '国家', value: detail.country },
                { key: '证件号', value: maskIdentityNumber(detail.id_number) },
                { key: '企业名称', value: detail.enterprise_name || '-' },
                { key: '统一社会信用代码', value: detail.business_registration_number || '-' },
                { key: '证件类型', value: documentTypeLabel(detail.document_type) },
                { key: '状态', value: <StatusTag value={detail.status} /> },
                { key: '目标等级', value: detail.target_kyc_level },
                { key: '提交时间', value: <TimestampText value={detail.submitted_at} /> },
                { key: '审核时间', value: <TimestampText value={detail.reviewed_at ?? null} /> }
              ]}
              size="medium"
            />
            <Divider margin="12px" />
            <Space align="start" spacing={12} style={{ width: '100%' }}>
              <Card bordered={false} shadows="hover" style={{ flex: 1 }}>
                <Space align="start" vertical style={{ width: '100%' }}>
                  <Text strong>证件正面</Text>
                  <Image alt="证件正面" height={220} imgStyle={{ objectFit: 'contain' }} preview src={detail.document_front_image} width="100%" />
                </Space>
              </Card>
              <Card bordered={false} shadows="hover" style={{ flex: 1 }}>
                <Space align="start" vertical style={{ width: '100%' }}>
                  <Text strong>证件反面</Text>
                  <Image alt="证件反面" height={220} imgStyle={{ objectFit: 'contain' }} preview src={detail.document_back_image} width="100%" />
                </Space>
              </Card>
              {detail.document_handheld_image ? (
                <Card bordered={false} shadows="hover" style={{ flex: 1 }}>
                  <Space align="start" vertical style={{ width: '100%' }}>
                    <Text strong>本人手持证件照</Text>
                    <Image alt="本人手持证件照" height={220} imgStyle={{ objectFit: 'contain' }} preview src={detail.document_handheld_image} width="100%" />
                  </Space>
                </Card>
              ) : null}
            </Space>

            {detail.status === 'pending' ? (
              <>
                <Divider margin="12px" />
                <Title heading={5}>审核处理</Title>
                <div className="admin-action-form admin-action-form-narrow">
                  <label>
                    通过后 KYC 等级
                    <AdminTextInput ariaLabel="通过后 KYC 等级" onChange={setReviewLevel} value={reviewLevel} />
                  </label>
                  <label>
                    审核原因
                    <AdminTextArea ariaLabel="审核原因" autosize onChange={setReviewReason} value={reviewReason} />
                  </label>
                </div>
                <Space>
                  <Button disabled={!reviewReason.trim()} loading={reviewing === 'approved'} onClick={() => reviewSubmission('approved')} theme="solid" type="primary">
                    审核通过
                  </Button>
                  <Button disabled={!reviewReason.trim()} loading={reviewing === 'rejected'} onClick={() => reviewSubmission('rejected')} type="danger">
                    审核拒绝
                  </Button>
                </Space>
              </>
            ) : (
              <Text type="secondary">审核人：{detail.reviewed_by ?? '-'}，审核原因：{detail.review_reason ?? '-'}</Text>
            )}
          </Space>
        ) : null}
      </SideSheet>
    </main>
  );
}
