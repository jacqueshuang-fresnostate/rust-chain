import { Banner, Button, Card, Space, Toast, Typography } from '@douyinfe/semi-ui';
import { useEffect, useState } from 'react';

import {
  confirmAdminTwoFactor,
  disableAdminTwoFactor,
  getAdminTwoFactorStatus,
  setupAdminTwoFactor,
  type AdminTwoFactorSetup
} from '../../api/adminAuth';
import { PageHeader } from '../../layouts/PageHeader';
import { AdminTextInput } from '../../shared/SemiFormControls';

const { Text, Title } = Typography;

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : '操作失败';
}

export function AdminTwoFactorPage() {
  const [enabled, setEnabled] = useState(false);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState(false);
  const [setup, setSetup] = useState<AdminTwoFactorSetup | null>(null);
  const [code, setCode] = useState('');

  useEffect(() => {
    getAdminTwoFactorStatus()
      .then((status) => setEnabled(status.totp_enabled))
      .catch((error) => Toast.error(errorMessage(error)))
      .finally(() => setLoading(false));
  }, []);

  const startSetup = async () => {
    setPending(true);
    try {
      setSetup(await setupAdminTwoFactor());
      setCode('');
    } catch (error) {
      Toast.error(errorMessage(error));
    } finally {
      setPending(false);
    }
  };

  const confirmSetup = async () => {
    setPending(true);
    try {
      const status = await confirmAdminTwoFactor(code.trim());
      setEnabled(status.totp_enabled);
      setSetup(null);
      setCode('');
      Toast.success('两步验证已开启');
    } catch (error) {
      Toast.error(errorMessage(error));
    } finally {
      setPending(false);
    }
  };

  const disable = async () => {
    setPending(true);
    try {
      const status = await disableAdminTwoFactor(code.trim());
      setEnabled(status.totp_enabled);
      setCode('');
      Toast.success('两步验证已关闭');
    } catch (error) {
      Toast.error(errorMessage(error));
    } finally {
      setPending(false);
    }
  };

  return (
    <div className="admin-action-workbench">
      <PageHeader title="管理员两步验证" description="为当前管理员账号绑定 TOTP 验证器，登录时需额外输入动态验证码。" />
      <Card loading={loading}>
        <Space vertical align="start" spacing={16} style={{ width: '100%' }}>
          <Banner
            type={enabled ? 'success' : 'warning'}
            description={enabled ? '当前账号已开启两步验证。' : '当前账号未开启两步验证，仅凭密码即可登录后台。'}
            closeIcon={null}
          />

          {!enabled && !setup ? (
            <Button theme="solid" loading={pending} onClick={startSetup}>
              开始绑定
            </Button>
          ) : null}

          {!enabled && setup ? (
            <Space vertical align="start" spacing={12} style={{ width: '100%' }}>
              <Title heading={6}>1. 在验证器中添加账号</Title>
              <Text copyable={{ content: setup.otpauth_uri }}>{setup.otpauth_uri}</Text>
              <Text type="tertiary">
                无法扫描时可手动输入密钥：<Text copyable={{ content: setup.secret }}>{setup.secret}</Text>
              </Text>
              <Title heading={6}>2. 输入验证器生成的 6 位验证码</Title>
              <label>
                验证码
                <AdminTextInput ariaLabel="验证码" value={code} onChange={setCode} />
              </label>
              <Space>
                <Button theme="solid" loading={pending} disabled={!code.trim()} onClick={confirmSetup}>
                  确认开启
                </Button>
                <Button theme="borderless" onClick={() => setSetup(null)}>
                  取消
                </Button>
              </Space>
              {/* 密钥仅在确认前展示；确认后无法再次取回，丢失验证器需要管理员直接改库恢复。 */}
              <Text type="warning">请先备份密钥：确认开启后无法再次查看，且系统当前没有自助找回入口。</Text>
            </Space>
          ) : null}

          {enabled ? (
            <Space vertical align="start" spacing={12}>
              <label>
                验证码
                <AdminTextInput ariaLabel="验证码" value={code} onChange={setCode} />
              </label>
              <Button type="danger" theme="solid" loading={pending} disabled={!code.trim()} onClick={disable}>
                关闭两步验证
              </Button>
            </Space>
          ) : null}
        </Space>
      </Card>
    </div>
  );
}
