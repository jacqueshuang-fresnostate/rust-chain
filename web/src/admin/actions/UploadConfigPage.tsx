import { Button, Card, Space, Toast, Typography } from '@douyinfe/semi-ui';
import { type ChangeEvent, useEffect, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';
import { AdminCheckbox, AdminPasswordInput, AdminSelect, AdminTextInput } from '../../shared/SemiFormControls';
import { StatusTag } from '../../shared/StatusTag';

const { Text, Title } = Typography;

const providerOptions = [
  { value: 'image_bed', label: '图床' },
  { value: 'oss', label: 'OSS' },
  { value: 's3', label: 'S3' },
  { value: 'local', label: '本地' }
];

const defaultConfigForm = {
  accessKey: '',
  allowedMimeTypes: 'image/png,image/jpeg,image/webp,image/gif',
  bearerToken: '',
  bucket: '',
  enabled: false,
  endpoint: '',
  fileField: 'file',
  keyPrefix: '',
  localRoot: '',
  maxFileSizeBytes: '10485760',
  provider: 'image_bed',
  publicBaseUrl: '',
  region: '',
  secretKey: ''
};

type UploadConfig = {
  access_key_mask?: string | null;
  access_key_set: boolean;
  allowed_mime_types: string[];
  bearer_token_mask?: string | null;
  bearer_token_set: boolean;
  bucket?: string | null;
  enabled: boolean;
  endpoint?: string | null;
  file_field?: string | null;
  id: number;
  key_prefix?: string | null;
  local_root?: string | null;
  max_file_size_bytes: number;
  name: string;
  provider: string;
  public_base_url?: string | null;
  region?: string | null;
  secret_key_set: boolean;
};

type SaveUploadConfigPayload = {
  access_key?: string;
  allowed_mime_types: string[];
  bearer_token?: string;
  bucket?: string;
  enabled: boolean;
  endpoint?: string;
  file_field?: string;
  key_prefix?: string;
  local_root?: string;
  max_file_size_bytes: number;
  provider: string;
  public_base_url?: string;
  reason: string;
  region?: string;
  secret_key?: string;
};

type UploadImageResponse = {
  delete_url?: string | null;
  download_url: string;
  mime_type: string;
  object_key: string;
  provider: string;
  share_url?: string | null;
  size_bytes: number;
};

type ConfigForm = typeof defaultConfigForm;

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

function formFromConfig(config: UploadConfig | null): ConfigForm {
  if (!config) {
    return defaultConfigForm;
  }

  return {
    accessKey: '',
    allowedMimeTypes: config.allowed_mime_types.join(','),
    bearerToken: '',
    bucket: config.bucket ?? '',
    enabled: config.enabled,
    endpoint: config.endpoint ?? '',
    fileField: config.file_field ?? 'file',
    keyPrefix: config.key_prefix ?? '',
    localRoot: config.local_root ?? '',
    maxFileSizeBytes: String(config.max_file_size_bytes),
    provider: config.provider,
    publicBaseUrl: config.public_base_url ?? '',
    region: config.region ?? '',
    secretKey: ''
  };
}

function addTrimmed(payload: SaveUploadConfigPayload, key: keyof SaveUploadConfigPayload, value: string) {
  const trimmed = value.trim();
  if (trimmed) {
    Object.assign(payload, { [key]: trimmed });
  }
}

function payloadFromForm(form: ConfigForm, reason: string): SaveUploadConfigPayload {
  const payload: SaveUploadConfigPayload = {
    allowed_mime_types: form.allowedMimeTypes.split(',').map((item) => item.trim()).filter(Boolean),
    enabled: form.enabled,
    max_file_size_bytes: Number.parseInt(form.maxFileSizeBytes, 10) || 0,
    provider: form.provider,
    reason
  };

  if (form.provider === 'image_bed') {
    addTrimmed(payload, 'endpoint', form.endpoint);
    addTrimmed(payload, 'file_field', form.fileField || 'file');
    addTrimmed(payload, 'bearer_token', form.bearerToken);
    return payload;
  }

  if (form.provider === 'local') {
    addTrimmed(payload, 'local_root', form.localRoot);
    addTrimmed(payload, 'public_base_url', form.publicBaseUrl);
    addTrimmed(payload, 'key_prefix', form.keyPrefix);
    return payload;
  }

  if (form.provider === 's3') {
    addTrimmed(payload, 'bucket', form.bucket);
    addTrimmed(payload, 'region', form.region);
    addTrimmed(payload, 'endpoint', form.endpoint);
    addTrimmed(payload, 'public_base_url', form.publicBaseUrl);
    addTrimmed(payload, 'key_prefix', form.keyPrefix);
    addTrimmed(payload, 'access_key', form.accessKey);
    addTrimmed(payload, 'secret_key', form.secretKey);
    return payload;
  }

  addTrimmed(payload, 'endpoint', form.endpoint);
  addTrimmed(payload, 'bucket', form.bucket);
  addTrimmed(payload, 'public_base_url', form.publicBaseUrl);
  addTrimmed(payload, 'key_prefix', form.keyPrefix);
  addTrimmed(payload, 'access_key', form.accessKey);
  addTrimmed(payload, 'secret_key', form.secretKey);
  return payload;
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

export function UploadConfigPage() {
  const [config, setConfig] = useState<UploadConfig | null>(null);
  const [configForm, setConfigForm] = useState<ConfigForm>(defaultConfigForm);
  const [loading, setLoading] = useState(true);
  const [testFile, setTestFile] = useState<File | null>(null);
  const [lastUpload, setLastUpload] = useState<UploadImageResponse | null>(null);

  async function loadConfig() {
    setLoading(true);
    try {
      const saved = await apiRequest<UploadConfig | null>('/admin/api/v1/upload/config');
      setConfig(saved);
      setConfigForm(formFromConfig(saved));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadConfig().catch((error) => Toast.error(errorMessage(error)));
  }, []);

  function handleTestFileChange(event: ChangeEvent<HTMLInputElement>) {
    setTestFile(event.target.files?.[0] ?? null);
  }

  async function uploadTestFile() {
    if (!testFile) {
      Toast.error('请选择测试上传文件');
      return;
    }
    const body = new FormData();
    body.append('file', testFile);
    await submitAction('测试上传', async () => {
      const response = await apiRequest<UploadImageResponse>('/admin/api/v1/uploads/images', { method: 'POST', body });
      setLastUpload(response);
    });
  }

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="上传配置" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>存储配置</Title>
            <div className="admin-action-form">
              <label>
                上传方式
                <AdminSelect
                  ariaLabel="上传方式"
                  onChange={(provider) => setConfigForm({ ...configForm, provider })}
                  optionList={providerOptions}
                  value={configForm.provider}
                />
              </label>
              {configForm.provider === 'image_bed' ? (
                <>
                  <label>
                    图床上传 endpoint
                    <AdminTextInput ariaLabel="图床上传 endpoint" value={configForm.endpoint} onChange={(endpoint) => setConfigForm({ ...configForm, endpoint })} />
                  </label>
                  <label>
                    文件字段名
                    <AdminTextInput ariaLabel="文件字段名" value={configForm.fileField} onChange={(fileField) => setConfigForm({ ...configForm, fileField })} />
                  </label>
                  <label>
                    图床 Bearer Token
                    <AdminPasswordInput ariaLabel="图床 Bearer Token" value={configForm.bearerToken} onChange={(bearerToken) => setConfigForm({ ...configForm, bearerToken })} />
                  </label>
                </>
              ) : null}
              {configForm.provider === 'local' ? (
                <>
                  <label>
                    本地存储目录
                    <AdminTextInput ariaLabel="本地存储目录" value={configForm.localRoot} onChange={(localRoot) => setConfigForm({ ...configForm, localRoot })} />
                  </label>
                  <label>
                    公开访问 base URL
                    <AdminTextInput ariaLabel="公开访问 base URL" value={configForm.publicBaseUrl} onChange={(publicBaseUrl) => setConfigForm({ ...configForm, publicBaseUrl })} />
                  </label>
                  <label>
                    Key 前缀
                    <AdminTextInput ariaLabel="Key 前缀" value={configForm.keyPrefix} onChange={(keyPrefix) => setConfigForm({ ...configForm, keyPrefix })} />
                  </label>
                </>
              ) : null}
              {configForm.provider === 's3' || configForm.provider === 'oss' ? (
                <>
                  <label>
                    Endpoint
                    <AdminTextInput ariaLabel="Endpoint" value={configForm.endpoint} onChange={(endpoint) => setConfigForm({ ...configForm, endpoint })} />
                  </label>
                  <label>
                    Bucket
                    <AdminTextInput ariaLabel="Bucket" value={configForm.bucket} onChange={(bucket) => setConfigForm({ ...configForm, bucket })} />
                  </label>
                  {configForm.provider === 's3' ? (
                    <label>
                      Region
                      <AdminTextInput ariaLabel="Region" value={configForm.region} onChange={(region) => setConfigForm({ ...configForm, region })} />
                    </label>
                  ) : null}
                  <label>
                    公开访问 base URL
                    <AdminTextInput ariaLabel="公开访问 base URL" value={configForm.publicBaseUrl} onChange={(publicBaseUrl) => setConfigForm({ ...configForm, publicBaseUrl })} />
                  </label>
                  <label>
                    Key 前缀
                    <AdminTextInput ariaLabel="Key 前缀" value={configForm.keyPrefix} onChange={(keyPrefix) => setConfigForm({ ...configForm, keyPrefix })} />
                  </label>
                  <label>
                    Access Key
                    <AdminPasswordInput ariaLabel="Access Key" value={configForm.accessKey} onChange={(accessKey) => setConfigForm({ ...configForm, accessKey })} />
                  </label>
                  <label>
                    Secret Key
                    <AdminPasswordInput ariaLabel="Secret Key" value={configForm.secretKey} onChange={(secretKey) => setConfigForm({ ...configForm, secretKey })} />
                  </label>
                </>
              ) : null}
              <label>
                最大文件大小
                <AdminTextInput ariaLabel="最大文件大小" type="number" value={configForm.maxFileSizeBytes} onChange={(maxFileSizeBytes) => setConfigForm({ ...configForm, maxFileSizeBytes })} />
              </label>
              <label>
                允许 MIME 类型
                <AdminTextInput ariaLabel="允许 MIME 类型" value={configForm.allowedMimeTypes} onChange={(allowedMimeTypes) => setConfigForm({ ...configForm, allowedMimeTypes })} />
              </label>
              <div className="admin-action-checkbox">
                <AdminCheckbox checked={configForm.enabled} onChange={(enabled) => setConfigForm({ ...configForm, enabled })}>
                  启用上传
                </AdminCheckbox>
              </div>
            </div>
            <Space>
              <ConfirmAction
                actionText="保存配置"
                title="确认保存上传配置"
                onConfirm={(reason) =>
                  submitAction('上传配置', async () => {
                    const saved = await apiRequest<UploadConfig>('/admin/api/v1/upload/config', {
                      method: 'PATCH',
                      body: JSON.stringify(payloadFromForm(configForm, reason))
                    });
                    setConfig(saved);
                    setConfigForm(formFromConfig(saved));
                  })
                }
              />
              <Button loading={loading} onClick={() => loadConfig().catch((error) => Toast.error(errorMessage(error)))}>
                刷新配置
              </Button>
            </Space>
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>配置状态</Title>
            <div className="admin-action-summary">
              <span>配置名称：{config?.name ?? '-'}</span>
              <span>上传方式：{config?.provider ?? '-'}</span>
              <span>启用状态：<StatusTag value={config?.enabled ?? false} /></span>
              <span>Bearer Token：{config?.bearer_token_mask ?? (config?.bearer_token_set ? '已设置' : '未设置')}</span>
              <span>Access Key：{config?.access_key_mask ?? (config?.access_key_set ? '已设置' : '未设置')}</span>
              <span>Secret Key：{config?.secret_key_set ? '已设置' : '未设置'}</span>
              <span>最大文件大小：{config?.max_file_size_bytes ?? '-'}</span>
            </div>
            <Text type="secondary">密钥输入框留空时保留已保存密文；切换上传目标时需重新输入密钥。</Text>
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>测试上传</Title>
            <div className="admin-action-form">
              <label>
                测试上传文件
                <input aria-label="测试上传文件" onChange={handleTestFileChange} type="file" />
              </label>
            </div>
            <Button onClick={uploadTestFile} type="primary">
              测试上传
            </Button>
            {lastUpload ? <Text type="secondary">最近上传 URL：{lastUpload.download_url}</Text> : null}
          </Space>
        </Card>
      </div>
    </main>
  );
}
