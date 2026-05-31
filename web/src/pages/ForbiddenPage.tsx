import { Button, Empty } from '@douyinfe/semi-ui';
import { Link } from 'react-router-dom';

export function ForbiddenPage() {
  return (
    <main className="exchange-page auth-result-page">
      <div>
        <Empty title="无权限" description="当前账号不能访问此页面" />
        <Link to="/login">
          <Button theme="solid" type="primary">
            返回登录
          </Button>
        </Link>
      </div>
    </main>
  );
}
