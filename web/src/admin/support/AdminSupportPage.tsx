import { hasAdminPermission, useAdminAccess } from '../access';
import { OnlineSupportWorkbench } from '../../support/OnlineSupportWorkbench';

export function AdminSupportPage() {
  const access = useAdminAccess();
  return (
    <OnlineSupportWorkbench
      canWrite={hasAdminPermission(access, 'support.conversations.write')}
      scope="admin"
    />
  );
}
