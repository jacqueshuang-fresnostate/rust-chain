import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { UploadConfigPage } from './UploadConfigPage';
import { apiRequest } from '../../api/client';

vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return {
    ...actual,
    apiRequest: vi.fn()
  };
});

const apiRequestMock = vi.mocked(apiRequest);

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

const imageBedConfig = {
  access_key_mask: null,
  access_key_set: false,
  allowed_mime_types: ['image/png', 'image/jpeg'],
  bearer_token_mask: 'tok****1234',
  bearer_token_set: true,
  bucket: null,
  enabled: true,
  endpoint: 'https://oss.example.test/api/v1/upload',
  file_field: 'file',
  id: 8,
  key_prefix: null,
  local_root: null,
  max_file_size_bytes: 10485760,
  name: 'default',
  provider: 'image_bed',
  public_base_url: null,
  region: null,
  secret_key_set: false
};

function semiSelectByLabel(label: string): HTMLElement {
  const labelNode = screen.getByText(label).closest('label') as HTMLElement | null;
  expect(labelNode).toBeInTheDocument();
  const select = labelNode?.querySelector('.semi-select') as HTMLElement | null;
  expect(select).toBeInTheDocument();
  return select as HTMLElement;
}

async function selectSemiOption(user: ReturnType<typeof userEvent.setup>, label: string, optionLabel: string) {
  await user.click(semiSelectByLabel(label));
  await waitFor(() => {
    expect([...document.querySelectorAll('.semi-select-option')].some((option) => option.textContent === optionLabel)).toBe(true);
  });
  const option = [...document.querySelectorAll('.semi-select-option')].find((item) => item.textContent === optionLabel) as HTMLElement | undefined;
  expect(option).toBeDefined();
  fireEvent.mouseEnter(option as HTMLElement);
  fireEvent.mouseDown(option as HTMLElement);
  fireEvent.mouseUp(option as HTMLElement);
  fireEvent.click(option as HTMLElement);
}

describe('UploadConfigPage', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
    apiRequestMock.mockReset();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/upload/config' && !init?.method) {
        return Promise.resolve(imageBedConfig);
      }
      if (path === '/admin/api/v1/upload/config' && init?.method === 'PATCH') {
        return Promise.resolve(imageBedConfig);
      }
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('loads redacted image-bed config without rendering plaintext secrets', async () => {
    render(<UploadConfigPage />);

    expect(await screen.findByDisplayValue('https://oss.example.test/api/v1/upload')).toBeInTheDocument();
    expect(screen.getByLabelText('图床 Bearer Token')).toHaveValue('');
    expect(screen.getByRole('checkbox', { name: '启用上传' })).toBeChecked();
    expect(screen.getByText(/Bearer Token：tok\*\*\*\*1234/)).toBeInTheDocument();
    expect(screen.getByText(/Secret Key：未设置/)).toBeInTheDocument();
    expect(screen.queryByText(/real-upload-token/)).not.toBeInTheDocument();
  });

  it('switches provider-specific fields', async () => {
    const user = userEvent.setup();
    render(<UploadConfigPage />);

    expect(await screen.findByLabelText('图床上传 endpoint')).toBeInTheDocument();
    await selectSemiOption(user, '上传方式', '本地');

    expect(screen.getByLabelText('本地存储目录')).toBeInTheDocument();
    expect(screen.getByLabelText('公开访问 base URL')).toBeInTheDocument();
    expect(screen.queryByLabelText('图床 Bearer Token')).not.toBeInTheDocument();

    await selectSemiOption(user, '上传方式', 'S3');

    expect(screen.getByLabelText('Bucket')).toBeInTheDocument();
    expect(screen.getByLabelText('Region')).toBeInTheDocument();
    expect(screen.getByLabelText('Access Key')).toBeInTheDocument();
    expect(screen.getByLabelText('Secret Key')).toBeInTheDocument();
  });

  it('saves image-bed config with reason and omits blank secrets', async () => {
    const user = userEvent.setup();
    render(<UploadConfigPage />);

    await user.clear(await screen.findByLabelText('图床上传 endpoint'));
    await user.type(screen.getByLabelText('图床上传 endpoint'), 'https://oss.new.test/api/v1/upload');
    await user.clear(screen.getByLabelText('文件字段名'));
    await user.type(screen.getByLabelText('文件字段名'), 'image');
    await user.clear(screen.getByLabelText('最大文件大小'));
    await user.type(screen.getByLabelText('最大文件大小'), '5242880');
    await user.click(screen.getByRole('button', { name: '保存配置' }));
    await user.type(screen.getByLabelText('操作原因'), 'configure uploads');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/upload/config',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/upload/config' && init?.method === 'PATCH')!;
    expect(JSON.parse(String(request?.body))).toEqual({
      allowed_mime_types: ['image/png', 'image/jpeg'],
      enabled: true,
      endpoint: 'https://oss.new.test/api/v1/upload',
      file_field: 'image',
      max_file_size_bytes: 5242880,
      provider: 'image_bed',
      reason: 'configure uploads'
    });
  });

  it('includes a newly entered image-bed token when saving', async () => {
    const user = userEvent.setup();
    render(<UploadConfigPage />);

    await user.type(await screen.findByLabelText('图床 Bearer Token'), 'new-token-value');
    await user.click(screen.getByRole('button', { name: '保存配置' }));
    await user.type(screen.getByLabelText('操作原因'), 'rotate upload token');
    await user.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/upload/config',
        expect.objectContaining({ method: 'PATCH' })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/upload/config' && init?.method === 'PATCH')!;
    expect(JSON.parse(String(request?.body))).toEqual(expect.objectContaining({ bearer_token: 'new-token-value' }));
  });

  it('uploads a test file with FormData and renders the returned URL', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path, init) => {
      if (path === '/admin/api/v1/upload/config' && !init?.method) {
        return Promise.resolve(imageBedConfig);
      }
      if (path === '/admin/api/v1/uploads/images' && init?.method === 'POST') {
        return Promise.resolve({
          delete_url: null,
          download_url: 'https://oss.example.test/file/result.png',
          mime_type: 'image/png',
          object_key: 'result.png',
          provider: 'image_bed',
          share_url: null,
          size_bytes: 8
        });
      }
      return Promise.resolve({});
    });

    render(<UploadConfigPage />);
    await screen.findByDisplayValue('https://oss.example.test/api/v1/upload');
    const file = new File(['png-data'], 'result.png', { type: 'image/png' });
    await user.upload(screen.getByLabelText('测试上传文件'), file);
    await user.click(screen.getByRole('button', { name: '测试上传' }));

    await waitFor(() => {
      expect(apiRequestMock).toHaveBeenCalledWith(
        '/admin/api/v1/uploads/images',
        expect.objectContaining({ method: 'POST', body: expect.any(FormData) })
      );
    });
    const [, request] = apiRequestMock.mock.calls.find(([path, init]) => path === '/admin/api/v1/uploads/images' && init?.method === 'POST')!;
    expect((request?.body as FormData).get('file')).toBe(file);
    expect(await screen.findByText(/https:\/\/oss\.example\.test\/file\/result\.png/)).toBeInTheDocument();
  });
});
