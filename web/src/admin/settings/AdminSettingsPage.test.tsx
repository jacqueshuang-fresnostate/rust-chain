import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createMemoryRouter, Link, RouterProvider } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';

import { ApiError } from '../../api/client';
import { AdminSettingsPage } from './AdminSettingsPage';
import {
  buildSettingsDifferences,
  buildSettingsImpactSummary,
  type SettingsFieldDefinition,
  validateSettingsFields
} from './differences';
import { adminSettingsQueryKeys, SETTINGS_CONFLICT_MESSAGE } from './query';
import { SettingsSaveConfirmation } from './SettingsSaveConfirmation';
import { useAdminSettingsEditor } from './useAdminSettingsEditor';

type TestConfig = {
  name: string;
  secret_set: boolean;
};

type TestForm = {
  name: string;
  secret: string;
  secretSet: boolean;
};

const testFields: ReadonlyArray<SettingsFieldDefinition<TestForm>> = [
  {
    key: 'name',
    field: '配置名称',
    impact: '保存后会影响测试配置消费者。',
    read: (config) => config.name,
    validate: (value) => String(value).trim() ? null : '配置名称不能为空。'
  },
  {
    key: 'secret',
    field: '访问密钥',
    impact: '保存后会影响测试配置消费者。',
    read: (config) => config.secret || (config.secretSet ? '__configured__' : ''),
    sensitive: true
  }
];

const testConfigApiPath = '/admin/api/v1/test-config';

type HarnessProps = {
  load: () => Promise<TestConfig>;
  save: (draft: TestForm, reason: string) => Promise<TestConfig>;
};

function savedConfig(draft: TestForm): TestConfig {
  return {
    name: draft.name,
    secret_set: draft.secretSet || Boolean(draft.secret)
  };
}

function SettingsHarness({ load, save }: HarnessProps) {
  const editor = useAdminSettingsEditor<TestConfig, TestForm>({
    initialForm: { name: '', secret: '', secretSet: false },
    load,
    save,
    selectForm: (config) => ({ name: config.name, secret: '', secretSet: config.secret_set }),
    settingKey: testConfigApiPath,
    successMessage: '测试配置已保存。'
  });
  const differences = buildSettingsDifferences(
    editor.baseline ?? editor.draft,
    editor.draft,
    testFields
  );
  const validationIssues = validateSettingsFields(editor.draft, testFields);
  const impactSummary = buildSettingsImpactSummary(
    differences,
    testFields,
    '保存后会影响测试配置消费者。'
  );

  return (
    <AdminSettingsPage
      feedback={editor.feedback}
      isDirty={editor.isDirty}
      isInitialLoading={editor.isInitialLoading}
      isReady={editor.isReady}
      isRefreshing={editor.isFetching}
      loadError={editor.loadError}
      onReload={editor.reloadLatest}
      title="测试配置"
    >
      <label>
        配置名称
        <input
          aria-label="配置名称"
          onChange={(event) => {
            const name = event.currentTarget.value;
            editor.setDraft((current) => ({ ...current, name }));
          }}
          value={editor.draft.name}
        />
      </label>
      <label>
        访问密钥
        <input
          aria-label="访问密钥"
          onChange={(event) => {
            const secret = event.currentTarget.value;
            editor.setDraft((current) => ({ ...current, secret }));
          }}
          value={editor.draft.secret}
        />
      </label>
      <SettingsSaveConfirmation
        actionText="保存测试配置"
        differences={differences}
        impactSummary={impactSummary}
        onConfirm={editor.saveChanges}
        title="确认保存测试配置"
        validationIssues={validationIssues}
      />
      <Link to="/other">离开测试配置</Link>
    </AdminSettingsPage>
  );
}

function renderHarness(load: HarnessProps['load'], save: HarnessProps['save']) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { gcTime: 0, retry: false },
      mutations: { retry: false }
    }
  });
  const router = createMemoryRouter(
    [
      { path: '/settings', element: <SettingsHarness load={load} save={save} /> },
      { path: '/other', element: <div>其他页面</div> }
    ],
    { initialEntries: ['/settings'] }
  );

  const view = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );

  return { queryClient, router, ...view };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe('AdminSettingsPage shared editor', () => {
  it('renders a unified loading state and retries transient query failures once', async () => {
    const pending = deferred<TestConfig>();
    const initialLoad = vi.fn(() => pending.promise);
    const save = vi.fn();
    const firstView = renderHarness(initialLoad, save);

    expect(screen.getByRole('status')).toHaveTextContent('正在加载配置');
    pending.resolve({ name: '初始配置', secret_set: true });
    expect(await screen.findByDisplayValue('初始配置')).toBeInTheDocument();
    firstView.unmount();

    const failedLoad = vi.fn<() => Promise<TestConfig>>().mockRejectedValue(new Error('网络暂时不可用'));
    renderHarness(failedLoad, save);

    expect(await screen.findByRole('alert')).toHaveTextContent('网络暂时不可用');
    expect(failedLoad).toHaveBeenCalledTimes(2);
  });

  it('shows Chinese field differences and impact without echoing sensitive values, then clears dirty state after success', async () => {
    const user = userEvent.setup();
    const load = vi.fn().mockResolvedValue({ name: '初始配置', secret_set: true });
    const save = vi.fn(async (draft: TestForm) => savedConfig(draft));
    const { queryClient } = renderHarness(load, save);

    await user.clear(await screen.findByLabelText('配置名称'));
    await user.type(screen.getByLabelText('配置名称'), '新配置');
    await user.clear(screen.getByLabelText('访问密钥'));
    await user.type(screen.getByLabelText('访问密钥'), 'TOKEN_NEW');

    expect(screen.getByRole('status')).toHaveTextContent('有未保存的变更');
    await user.click(screen.getByRole('button', { name: '保存测试配置' }));

    expect(await screen.findByText('字段差异（2 项）')).toBeInTheDocument();
    expect(screen.getAllByText('配置名称').length).toBeGreaterThan(0);
    expect(screen.getAllByText('访问密钥').length).toBeGreaterThan(0);
    expect(screen.getByText('保存后会影响测试配置消费者。')).toBeInTheDocument();
    expect(screen.queryByText('TOKEN_OLD')).not.toBeInTheDocument();
    expect(screen.queryByText('TOKEN_NEW')).not.toBeInTheDocument();
    expect(screen.getByText('已配置（内容不回显）')).toBeInTheDocument();
    expect(screen.getByText('已更新（内容不回显）')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '确认保存' })).toBeDisabled();

    await user.type(screen.getByLabelText('操作原因'), '  配置升级  ');
    await user.click(screen.getByRole('button', { name: '确认保存' }));

    await waitFor(() => {
      expect(save).toHaveBeenCalledWith(
        { name: '新配置', secret: 'TOKEN_NEW', secretSet: true },
        '配置升级'
      );
    });
    expect(await screen.findByText('测试配置已保存。')).toBeInTheDocument();
    expect(screen.queryByText('有未保存的变更')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '保存测试配置' })).toBeDisabled();
    expect(queryClient.getQueryState(adminSettingsQueryKeys.detail(testConfigApiPath))?.isInvalidated).toBe(true);
  });

  it('uses the shared field schema to block invalid settings with a Chinese field error', async () => {
    const user = userEvent.setup();
    const load = vi.fn().mockResolvedValue({ name: '初始配置', secret_set: false });
    const save = vi.fn(async (draft: TestForm) => savedConfig(draft));
    renderHarness(load, save);

    await user.clear(await screen.findByLabelText('配置名称'));

    expect(screen.getByRole('list', { name: '配置校验错误' })).toHaveTextContent(
      '配置名称：配置名称不能为空。'
    );
    expect(screen.getByRole('button', { name: '保存测试配置' })).toBeDisabled();
    expect(save).not.toHaveBeenCalled();
  });

  it('blocks beforeunload and in-app navigation while dirty, then releases both guards after saving', async () => {
    const user = userEvent.setup();
    const load = vi.fn().mockResolvedValue({ name: '初始配置', secret_set: false });
    const save = vi.fn(async (draft: TestForm) => savedConfig(draft));
    const { router } = renderHarness(load, save);

    await user.clear(await screen.findByLabelText('配置名称'));
    await user.type(screen.getByLabelText('配置名称'), '待保存配置');

    const beforeUnload = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(beforeUnload);
    expect(beforeUnload.defaultPrevented).toBe(true);

    await user.click(screen.getByRole('link', { name: '离开测试配置' }));
    expect(await screen.findByText('你有未保存的更改，离开当前页面将丢失这些内容。')).toBeInTheDocument();
    expect(router.state.location.pathname).toBe('/settings');
    await user.click(screen.getByRole('button', { name: '继续编辑' }));
    expect(router.state.location.pathname).toBe('/settings');

    await user.click(screen.getByRole('button', { name: '保存测试配置' }));
    await user.type(await screen.findByLabelText('操作原因'), '完成编辑');
    await user.click(screen.getByRole('button', { name: '确认保存' }));
    expect(await screen.findByText('测试配置已保存。')).toBeInTheDocument();

    await waitFor(() => {
      const afterSaveBeforeUnload = new Event('beforeunload', { cancelable: true });
      window.dispatchEvent(afterSaveBeforeUnload);
      expect(afterSaveBeforeUnload.defaultPrevented).toBe(false);
    });

    await router.navigate('/other');
    expect(await screen.findByText('其他页面')).toBeInTheDocument();
    expect(screen.queryByText('确认离开当前页面')).not.toBeInTheDocument();
  });

  it('continues a blocked in-app navigation only after explicitly discarding the draft', async () => {
    const user = userEvent.setup();
    const load = vi.fn().mockResolvedValue({ name: '初始配置', secret_set: false });
    const save = vi.fn(async (draft: TestForm) => savedConfig(draft));
    const { router } = renderHarness(load, save);

    await user.clear(await screen.findByLabelText('配置名称'));
    await user.type(screen.getByLabelText('配置名称'), '准备放弃的草稿');
    await user.click(screen.getByRole('link', { name: '离开测试配置' }));

    expect(router.state.location.pathname).toBe('/settings');
    await user.click(await screen.findByRole('button', { name: '放弃未保存更改并离开' }));
    expect(await screen.findByText('其他页面')).toBeInTheDocument();
    expect(router.state.location.pathname).toBe('/other');
  });

  it('keeps the local draft on 409, invalidates the shared key, and reloads only after Chinese confirmation', async () => {
    const user = userEvent.setup();
    const load = vi.fn().mockResolvedValueOnce({ name: '旧配置', secret_set: false });
    const save = vi.fn().mockRejectedValue(new ApiError(409, 'CONFIG_CONFLICT', 'stale revision'));
    const { queryClient } = renderHarness(load, save);

    await user.clear(await screen.findByLabelText('配置名称'));
    await user.type(screen.getByLabelText('配置名称'), '本地草稿');
    await user.click(screen.getByRole('button', { name: '保存测试配置' }));
    await user.type(await screen.findByLabelText('操作原因'), '尝试保存');
    await user.click(screen.getByRole('button', { name: '确认保存' }));

    await waitFor(() => expect(screen.getAllByText(SETTINGS_CONFLICT_MESSAGE).length).toBeGreaterThan(0));
    expect(screen.getByLabelText('配置名称')).toHaveValue('本地草稿');
    expect(queryClient.getQueryState(adminSettingsQueryKeys.detail(testConfigApiPath))?.isInvalidated).toBe(true);

    await user.click(screen.getByRole('button', { name: '取消保存' }));
    load.mockResolvedValueOnce({ name: '其他管理员的新配置', secret_set: false });
    await user.click(screen.getByRole('button', { name: '重新加载最新配置' }));
    expect(await screen.findByText('当前修改尚未保存。重新加载最新配置会丢弃这些更改，是否继续？')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '放弃未保存更改并重新加载' }));

    expect(await screen.findByDisplayValue('其他管理员的新配置')).toBeInTheDocument();
    expect(screen.queryByText('有未保存的变更')).not.toBeInTheDocument();
  });

  it('keeps the modal and draft after a save error and retries only after another explicit confirmation', async () => {
    const user = userEvent.setup();
    const load = vi.fn().mockResolvedValue({ name: '初始配置', secret_set: false });
    const save = vi
      .fn<(draft: TestForm, reason: string) => Promise<TestConfig>>()
      .mockRejectedValueOnce(new ApiError(
        503,
        'SERVICE_UNAVAILABLE',
        '保存服务暂时不可用，token=raw-error-token\nprivate backend stack'
      ))
      .mockImplementationOnce(async (draft) => savedConfig(draft));
    renderHarness(load, save);

    await user.clear(await screen.findByLabelText('配置名称'));
    await user.type(screen.getByLabelText('配置名称'), '等待重试的草稿');
    await user.click(screen.getByRole('button', { name: '保存测试配置' }));
    await user.type(await screen.findByLabelText('操作原因'), '人工重试保存');
    await user.click(screen.getByRole('button', { name: '确认保存' }));

    await waitFor(() => {
      expect(screen.getAllByText('保存服务暂时不可用，token=***').length).toBeGreaterThan(0);
    });
    expect(document.body).not.toHaveTextContent('raw-error-token');
    expect(document.body).not.toHaveTextContent('private backend stack');
    expect(save).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText('配置名称')).toHaveValue('等待重试的草稿');
    expect(screen.getByLabelText('操作原因')).toHaveValue('人工重试保存');
    expect(screen.getByText('有未保存的变更')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '确认保存' }));
    expect(await screen.findByText('测试配置已保存。')).toBeInTheDocument();
    expect(save).toHaveBeenCalledTimes(2);
    expect(screen.queryByText('有未保存的变更')).not.toBeInTheDocument();
  });

  it('does not retry a 4xx load and exposes a retry action', async () => {
    const user = userEvent.setup();
    const load = vi.fn<() => Promise<TestConfig>>().mockRejectedValueOnce(new ApiError(403, 'FORBIDDEN', '没有读取权限'));
    const save = vi.fn();
    renderHarness(load, save);

    expect(await screen.findByRole('alert')).toHaveTextContent('没有读取权限');
    expect(load).toHaveBeenCalledTimes(1);

    load.mockResolvedValueOnce({ name: '恢复后的配置', secret_set: false });
    await user.click(screen.getByRole('button', { name: '重试加载' }));
    expect(await screen.findByDisplayValue('恢复后的配置')).toBeInTheDocument();
  });
});
