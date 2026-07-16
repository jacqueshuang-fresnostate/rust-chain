import { IconCamera, IconDelete, IconUpload } from '@douyinfe/semi-icons';
import { Avatar, Button, Image, Space, Toast, Typography, Upload } from '@douyinfe/semi-ui';
import type { FileItem, customRequestArgs } from '@douyinfe/semi-ui/lib/es/upload';

import { ApiError, apiRequest } from '../api/client';

const { Text } = Typography;

export type AdminUploadedImage = {
  delete_url?: string | null;
  download_url: string;
  mime_type: string;
  object_key: string;
  provider: string;
  share_url?: string | null;
  size_bytes: number;
};

type AdminImageUploadProps = {
  buttonText?: string;
  label: string;
  onChange: (url: string) => void;
  onUploaded?: (image: AdminUploadedImage) => void;
  value: string;
  variant?: 'avatar' | 'banner' | 'picture';
};

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '上传失败';
}

export async function uploadAdminImageFile(file: File): Promise<AdminUploadedImage> {
  const body = new FormData();
  body.append('file', file);
  return apiRequest<AdminUploadedImage>('/admin/api/v1/uploads/images', { method: 'POST', body });
}

function uploadedFileItem(url: string, label: string): FileItem {
  const name = decodeURIComponent(url.split('/').filter(Boolean).at(-1) ?? `${label}.png`);
  return {
    name,
    preview: true,
    size: '',
    status: 'success',
    uid: `uploaded-${url}`,
    url
  };
}

const avatarHoverMask = (
  <span
    style={{
      alignItems: 'center',
      backgroundColor: 'var(--semi-color-overlay-bg)',
      color: 'var(--semi-color-white)',
      display: 'flex',
      height: '100%',
      justifyContent: 'center',
      width: '100%'
    }}
  >
    <IconCamera aria-hidden="true" />
  </span>
);

export function AdminImageUpload({ buttonText = '上传图片', label, onChange, onUploaded, value, variant = 'picture' }: AdminImageUploadProps) {
  const currentUrl = value.trim();
  const defaultFileList = currentUrl ? [uploadedFileItem(currentUrl, label)] : [];

  const customRequest = async (request: customRequestArgs) => {
    try {
      request.onProgress({ loaded: 1, total: 2 });
      const response = await uploadAdminImageFile(request.fileInstance);
      onChange(response.download_url);
      onUploaded?.(response);
      request.onSuccess(response);
    } catch (error) {
      Toast.error(errorMessage(error));
      request.onError({ status: error instanceof ApiError ? error.status : 500 });
    }
  };

  if (variant === 'avatar') {
    return (
      <Space align="start" spacing={8} vertical style={{ width: '100%' }}>
        <Text strong>{label}</Text>
        <Space align="center" spacing={8}>
          <Upload
            accept="image/png,image/jpeg,image/webp,image/gif"
            action="/admin/api/v1/uploads/images"
            customRequest={customRequest}
            key={currentUrl || 'empty-avatar'}
            limit={1}
            onSizeError={() => Toast.error('图片大小超过上传配置限制')}
            showUploadList={false}
          >
            <Avatar alt={label} hoverMask={avatarHoverMask} shape="square" size="large" src={currentUrl || undefined}>
              <IconCamera aria-hidden="true" />
            </Avatar>
          </Upload>
          {currentUrl ? <Button aria-label={`清除${label}`} icon={<IconDelete aria-hidden="true" />} onClick={() => onChange('')} size="small" theme="borderless" type="danger" /> : null}
        </Space>
      </Space>
    );
  }

  return (
    <Space align="start" spacing={8} vertical style={{ width: '100%' }}>
      <Text strong>{label}</Text>
      <Upload
        accept="image/png,image/jpeg,image/webp,image/gif"
        action="/admin/api/v1/uploads/images"
        customRequest={customRequest}
        defaultFileList={defaultFileList}
        key={currentUrl || 'empty'}
        limit={1}
        listType="picture"
        onRemove={() => onChange('')}
        onSizeError={() => Toast.error('图片大小超过上传配置限制')}
        picHeight={variant === 'banner' ? 96 : 72}
        picWidth={variant === 'banner' ? 240 : 120}
        showReplace
      >
        <Button icon={<IconUpload aria-hidden="true" />}>{buttonText}</Button>
      </Upload>
    </Space>
  );
}

export function AdminImageCell({ alt = '图片', value }: { alt?: string; value: unknown }) {
  if (typeof value !== 'string' || !value.trim()) {
    return <span>-</span>;
  }

  return <Image alt={alt} height={36} imgStyle={{ objectFit: 'cover' }} preview src={value.trim()} width={36} />;
}
