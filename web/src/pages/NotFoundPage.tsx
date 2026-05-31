import { Button, Empty } from '@douyinfe/semi-ui';
import { Link } from 'react-router-dom';

export function NotFoundPage() {
  return (
    <main className="exchange-page auth-result-page">
      <div>
        <Empty title="页面不存在" description="请检查访问地址" />
        <Link to="/login">
          <Button>返回登录</Button>
        </Link>
      </div>
    </main>
  );
}
