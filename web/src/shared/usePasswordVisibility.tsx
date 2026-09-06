import { IconEyeClosedSolid, IconEyeOpened } from '@douyinfe/semi-icons';
import { Button } from '@douyinfe/semi-ui';
import { useState } from 'react';

/** Semi's built-in password-mode eye has hard-coded English accessible labels. */
export function usePasswordVisibility(disabled = false) {
  const [visible, setVisible] = useState(false);
  const label = visible ? '隐藏密码' : '显示密码';
  return {
    type: visible ? 'text' : 'password',
    suffix: disabled ? undefined : <Button
      aria-label={label}
      title={label}
      htmlType="button"
      theme="borderless"
      icon={visible ? <IconEyeOpened aria-hidden="true" /> : <IconEyeClosedSolid aria-hidden="true" />}
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => setVisible((current) => !current)}
    />
  };
}
