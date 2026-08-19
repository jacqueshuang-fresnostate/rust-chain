import {
  IconLockStroked,
  IconRefresh,
  IconSearch,
  IconSend,
  IconTickCircle,
  IconUnlockStroked
} from '@douyinfe/semi-icons';
import {
  Badge,
  Banner,
  Button,
  Card,
  Empty,
  Input,
  Space,
  Spin,
  Tag,
  TextArea,
  Toast,
  Typography
} from '@douyinfe/semi-ui';
import type { ColumnProps } from '@douyinfe/semi-ui/lib/es/table';
import {
  type FormEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState
} from 'react';

import {
  createStaffSupportApi,
  type StaffSupportApi,
  type StaffSupportScope,
  type SupportConversation,
  type SupportConversationStatus,
  type SupportMessage,
  type SupportReplyInput
} from '../api/support';
import { PageHeader } from '../layouts/PageHeader';
import { DataTable, DEFAULT_PAGE_SIZE } from '../shared/DataTable';
import { TimestampText } from '../shared/TimestampText';

const { Text, Title } = Typography;

export const DEFAULT_SUPPORT_POLL_INTERVAL_MS = 10_000;
const SUPPORT_MESSAGE_LIMIT = 100;

export type SupportQueueStatusFilter = 'all' | SupportConversationStatus;
export type SupportAssignmentFilter = 'all' | 'unassigned';

type OnlineSupportWorkbenchProps = {
  api?: StaffSupportApi;
  canWrite?: boolean;
  pollIntervalMs?: number;
  scope: StaffSupportScope;
};

type FailedReply = SupportReplyInput & {
  conversationId: number;
  error: Error;
};

type ConversationAction = 'read' | 'status' | null;

let fallbackMessageIdSequence = 0;

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error('请求失败，请稍后重试');
}

function scalarLength(value: string): number {
  return Array.from(value).length;
}

function truncateScalars(value: string, limit: number): string {
  return Array.from(value).slice(0, limit).join('');
}

export function createSupportClientMessageId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `web-${crypto.randomUUID()}`;
  }

  fallbackMessageIdSequence += 1;
  return `web-${Date.now().toString(36)}-${fallbackMessageIdSequence.toString(36)}`;
}

export function supportAssignmentLabel(conversation: SupportConversation): string {
  const code = conversation.assigned_agent_code?.trim();
  const id = conversation.assigned_agent_id;
  if (code && typeof id === 'number') {
    return `${code}（ID ${id}）`;
  }
  if (code) {
    return code;
  }
  if (typeof id === 'number') {
    return `代理 ID ${id}`;
  }
  return '未分配';
}

function conversationSearchText(conversation: SupportConversation): string {
  return [
    conversation.id,
    conversation.user_id,
    conversation.email,
    conversation.phone,
    conversation.assigned_agent_id,
    conversation.assigned_agent_code,
    conversation.last_message_preview
  ]
    .filter((value) => value !== null && value !== undefined)
    .join(' ')
    .toLocaleLowerCase('zh-CN');
}

export function filterSupportConversations(
  conversations: SupportConversation[],
  status: SupportQueueStatusFilter,
  keyword: string,
  assignment: SupportAssignmentFilter
): SupportConversation[] {
  const normalizedKeyword = keyword.trim().toLocaleLowerCase('zh-CN');
  return conversations.filter((conversation) => {
    if (status !== 'all' && conversation.status !== status) {
      return false;
    }
    if (assignment === 'unassigned' && conversation.assigned_agent_id !== null && conversation.assigned_agent_id !== undefined) {
      return false;
    }
    return !normalizedKeyword || conversationSearchText(conversation).includes(normalizedKeyword);
  });
}

function supportStatusTag(status: SupportConversationStatus) {
  return status === 'closed' ? <Tag color="grey">已关闭</Tag> : <Tag color="green">进行中</Tag>;
}

function messageSenderLabel(message: SupportMessage): string {
  if (message.sender_type === 'user') {
    return '客户';
  }
  if (message.sender_type === 'agent') {
    return '代理';
  }
  if (message.sender_type === 'admin') {
    return '管理员';
  }
  return message.sender_type;
}

function contactLabel(conversation: SupportConversation): string {
  return conversation.email?.trim() || conversation.phone?.trim() || '未提供联系方式';
}

function isStaffMessage(message: SupportMessage): boolean {
  return message.sender_type === 'agent' || message.sender_type === 'admin';
}

export function mergeSupportMessages(
  current: readonly SupportMessage[],
  incoming: readonly SupportMessage[]
): SupportMessage[] {
  const byId = new Map<number, SupportMessage>();
  current.forEach((message) => byId.set(message.id, message));
  incoming.forEach((message) => byId.set(message.id, message));
  return [...byId.values()].sort((left, right) => (
    left.created_at - right.created_at || left.id - right.id
  ));
}

export function OnlineSupportWorkbench({
  api: providedApi,
  canWrite = true,
  pollIntervalMs = DEFAULT_SUPPORT_POLL_INTERVAL_MS,
  scope
}: OnlineSupportWorkbenchProps) {
  const defaultApi = useMemo(() => createStaffSupportApi(scope), [scope]);
  const api = providedApi ?? defaultApi;
  const mountedRef = useRef(true);
  const queueRequestVersion = useRef(0);
  const detailRequestVersion = useRef(0);
  const olderMessagesRequestVersion = useRef(0);
  const selectedConversationIdRef = useRef<number | null>(null);
  const historyPaginationRef = useRef({
    conversationId: null as number | null,
    initialized: false,
    hasMore: false,
    nextBeforeId: null as number | null
  });
  const [statusFilter, setStatusFilter] = useState<SupportQueueStatusFilter>('open');
  const [assignmentFilter, setAssignmentFilter] = useState<SupportAssignmentFilter>('all');
  const [queuePage, setQueuePage] = useState(1);
  const [queuePageSize, setQueuePageSize] = useState(DEFAULT_PAGE_SIZE);
  const [keywordDraft, setKeywordDraft] = useState('');
  const [keyword, setKeyword] = useState('');
  const [conversations, setConversations] = useState<SupportConversation[]>([]);
  const [conversationTotal, setConversationTotal] = useState(0);
  const [queueLoading, setQueueLoading] = useState(true);
  const [queueError, setQueueError] = useState<Error | null>(null);
  const [lastReconciledAt, setLastReconciledAt] = useState<number | null>(null);
  const [selectedConversationId, setSelectedConversationId] = useState<number | null>(null);
  const [conversationDetail, setConversationDetail] = useState<SupportConversation | null>(null);
  const [messages, setMessages] = useState<SupportMessage[]>([]);
  const [historyHasMore, setHistoryHasMore] = useState(false);
  const [historyNextBeforeId, setHistoryNextBeforeId] = useState<number | null>(null);
  const [olderMessagesLoading, setOlderMessagesLoading] = useState(false);
  const [olderMessagesError, setOlderMessagesError] = useState<Error | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<Error | null>(null);
  const [composerDraft, setComposerDraft] = useState('');
  const [replySending, setReplySending] = useState(false);
  const [failedReply, setFailedReply] = useState<FailedReply | null>(null);
  const [conversationAction, setConversationAction] = useState<ConversationAction>(null);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      queueRequestVersion.current += 1;
      detailRequestVersion.current += 1;
      olderMessagesRequestVersion.current += 1;
    };
  }, []);

  selectedConversationIdRef.current = selectedConversationId;

  const applyConversation = useCallback((conversation: SupportConversation) => {
    setConversations((current) => {
      const present = current.some((candidate) => candidate.id === conversation.id);
      return present
        ? current.map((candidate) => candidate.id === conversation.id ? conversation : candidate)
        : current;
    });
    setConversationDetail((current) => {
      if (!current || current.id !== conversation.id) {
        return current;
      }
      return conversation;
    });
  }, []);

  const loadQueue = useCallback(async ({ silent = false }: { silent?: boolean } = {}) => {
    const requestVersion = ++queueRequestVersion.current;
    if (!silent) {
      setQueueLoading(true);
    }

    try {
      const result = await api.listConversations({
        status: statusFilter === 'all' ? undefined : statusFilter,
        unassigned: scope === 'admin' && assignmentFilter === 'unassigned' ? true : undefined,
        limit: queuePageSize,
        offset: (queuePage - 1) * queuePageSize
      });
      if (!mountedRef.current || requestVersion !== queueRequestVersion.current) {
        return;
      }

      setConversations(Array.isArray(result.conversations) ? result.conversations : []);
      setConversationTotal(typeof result.total === 'number' ? result.total : result.conversations.length);
      setQueueError(null);
      setLastReconciledAt(Date.now());
    } catch (error) {
      if (mountedRef.current && requestVersion === queueRequestVersion.current) {
        setQueueError(asError(error));
      }
    } finally {
      if (mountedRef.current && requestVersion === queueRequestVersion.current) {
        setQueueLoading(false);
      }
    }
  }, [api, assignmentFilter, queuePage, queuePageSize, scope, statusFilter]);

  const loadConversation = useCallback(async (
    conversationId: number,
    { silent = false }: { silent?: boolean } = {}
  ) => {
    const requestVersion = ++detailRequestVersion.current;
    if (!silent) {
      setDetailLoading(true);
    }

    try {
      const [detail, messageList] = await Promise.all([
        api.getConversation(conversationId),
        api.getMessages(conversationId, { limit: SUPPORT_MESSAGE_LIMIT })
      ]);
      if (!mountedRef.current || requestVersion !== detailRequestVersion.current) {
        return;
      }

      setConversationDetail(detail);
      setMessages((current) => mergeSupportMessages(current, messageList.messages));
      const history = historyPaginationRef.current;
      if (history.conversationId !== conversationId || !history.initialized) {
        historyPaginationRef.current = {
          conversationId,
          initialized: true,
          hasMore: messageList.has_more && messageList.next_before_id !== null,
          nextBeforeId: messageList.has_more ? messageList.next_before_id : null
        };
        setHistoryHasMore(historyPaginationRef.current.hasMore);
        setHistoryNextBeforeId(historyPaginationRef.current.nextBeforeId);
        setOlderMessagesError(null);
      }
      setDetailError(null);
      applyConversation(detail);
    } catch (error) {
      if (mountedRef.current && requestVersion === detailRequestVersion.current) {
        setDetailError(asError(error));
      }
    } finally {
      if (mountedRef.current && requestVersion === detailRequestVersion.current) {
        setDetailLoading(false);
      }
    }
  }, [api, applyConversation]);

  useEffect(() => {
    void loadQueue();
  }, [loadQueue]);

  useEffect(() => {
    if (selectedConversationId === null) {
      setConversationDetail(null);
      setMessages([]);
      setDetailError(null);
      setHistoryHasMore(false);
      setHistoryNextBeforeId(null);
      setOlderMessagesLoading(false);
      setOlderMessagesError(null);
      historyPaginationRef.current = {
        conversationId: null,
        initialized: false,
        hasMore: false,
        nextBeforeId: null
      };
      return;
    }

    olderMessagesRequestVersion.current += 1;
    setConversationDetail(null);
    setMessages([]);
    setDetailError(null);
    setHistoryHasMore(false);
    setHistoryNextBeforeId(null);
    setOlderMessagesLoading(false);
    setOlderMessagesError(null);
    historyPaginationRef.current = {
      conversationId: selectedConversationId,
      initialized: false,
      hasMore: false,
      nextBeforeId: null
    };
    setComposerDraft('');
    setFailedReply(null);
    void loadConversation(selectedConversationId);
  }, [loadConversation, selectedConversationId]);

  useEffect(() => {
    if (!Number.isFinite(pollIntervalMs) || pollIntervalMs <= 0) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadQueue({ silent: true });
      if (selectedConversationId !== null) {
        void loadConversation(selectedConversationId, { silent: true });
      }
    }, pollIntervalMs);

    return () => window.clearInterval(intervalId);
  }, [loadConversation, loadQueue, pollIntervalMs, selectedConversationId]);

  const visibleConversations = useMemo(
    () => filterSupportConversations(conversations, statusFilter, keyword, assignmentFilter),
    [assignmentFilter, conversations, keyword, statusFilter]
  );
  const selectedConversation = conversationDetail
    ?? conversations.find((conversation) => conversation.id === selectedConversationId)
    ?? null;
  const sortedMessages = useMemo(
    () => mergeSupportMessages([], messages),
    [messages]
  );
  const composerLength = scalarLength(composerDraft);

  const queueColumns = useMemo<Array<ColumnProps<SupportConversation>>>(() => [
    {
      dataIndex: 'user_id',
      key: 'customer',
      title: '客户',
      width: 190,
      render: (_value, conversation) => (
        <span className="support-customer-cell">
          <Text strong>用户 {conversation.user_id}</Text>
          <Text className="support-cell-ellipsis" title={contactLabel(conversation)} type="tertiary">
            {contactLabel(conversation)}
          </Text>
        </span>
      )
    },
    {
      dataIndex: 'assigned_agent_id',
      key: 'assignment',
      title: '当前归属代理',
      width: 180,
      render: (_value, conversation) => (
        <Text className={conversation.assigned_agent_id == null ? 'support-unassigned-label' : undefined}>
          {supportAssignmentLabel(conversation)}
        </Text>
      )
    },
    {
      dataIndex: 'status',
      key: 'status',
      title: '会话状态',
      width: 110,
      render: (_value, conversation) => supportStatusTag(conversation.status)
    },
    {
      dataIndex: 'staff_unread_count',
      key: 'unread',
      title: '客服未读',
      width: 110,
      render: (_value, conversation) => conversation.staff_unread_count > 0
        ? (
            <Badge count={conversation.staff_unread_count} overflowCount={99} type="danger">
              <span className="support-unread-anchor">新消息</span>
            </Badge>
          )
        : <Text type="tertiary">无未读</Text>
    },
    {
      dataIndex: 'last_message_preview',
      key: 'preview',
      title: '最近消息',
      width: 220,
      render: (_value, conversation) => (
        <Text
          className="support-cell-ellipsis"
          title={conversation.last_message_preview?.trim() || '暂无消息摘要'}
          type={conversation.last_message_preview ? 'primary' : 'tertiary'}
        >
          {conversation.last_message_preview?.trim() || '暂无消息摘要'}
        </Text>
      )
    },
    {
      dataIndex: 'last_message_at',
      key: 'last_message_at',
      title: '最近消息时间',
      width: 180,
      render: (_value, conversation) => <TimestampText value={conversation.last_message_at} />
    },
    {
      dataIndex: 'id',
      key: 'actions',
      title: '操作',
      width: 120,
      fixed: 'right',
      render: (_value, conversation) => (
        <Button
          aria-label={`打开用户 ${conversation.user_id} 会话`}
          onClick={() => setSelectedConversationId(conversation.id)}
          size="small"
          theme={selectedConversationId === conversation.id ? 'solid' : 'borderless'}
          type="primary"
        >
          {selectedConversationId === conversation.id ? '当前会话' : '打开会话'}
        </Button>
      )
    }
  ], [selectedConversationId]);

  function submitKeyword(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setKeyword(keywordDraft.trim());
  }

  async function loadOlderMessages() {
    const conversationId = selectedConversationIdRef.current;
    const history = historyPaginationRef.current;
    if (
      conversationId === null
      || history.conversationId !== conversationId
      || !history.hasMore
      || history.nextBeforeId === null
      || olderMessagesLoading
    ) {
      return;
    }

    const requestVersion = ++olderMessagesRequestVersion.current;
    setOlderMessagesLoading(true);
    setOlderMessagesError(null);
    try {
      const page = await api.getMessages(conversationId, {
        before_id: history.nextBeforeId,
        limit: SUPPORT_MESSAGE_LIMIT
      });
      if (
        !mountedRef.current
        || requestVersion !== olderMessagesRequestVersion.current
        || selectedConversationIdRef.current !== conversationId
      ) {
        return;
      }

      setMessages((current) => mergeSupportMessages(current, page.messages));
      const nextHistory = {
        conversationId,
        initialized: true,
        hasMore: page.has_more && page.next_before_id !== null,
        nextBeforeId: page.has_more ? page.next_before_id : null
      };
      historyPaginationRef.current = nextHistory;
      setHistoryHasMore(nextHistory.hasMore);
      setHistoryNextBeforeId(nextHistory.nextBeforeId);
    } catch (error) {
      if (
        mountedRef.current
        && requestVersion === olderMessagesRequestVersion.current
        && selectedConversationIdRef.current === conversationId
      ) {
        setOlderMessagesError(asError(error));
      }
    } finally {
      if (
        mountedRef.current
        && requestVersion === olderMessagesRequestVersion.current
        && selectedConversationIdRef.current === conversationId
      ) {
        setOlderMessagesLoading(false);
      }
    }
  }

  async function runReadAction() {
    if (!selectedConversation || !canWrite || conversationAction) {
      return;
    }

    const messageId = selectedConversation.last_message_id ?? sortedMessages.at(-1)?.id;
    if (!messageId) {
      Toast.warning('当前会话没有可标记的消息');
      return;
    }

    setConversationAction('read');
    try {
      const result = await api.markRead(selectedConversation.id, messageId);
      applyConversation(result);
      Toast.success('会话已标记为已读');
      void loadQueue({ silent: true });
      void loadConversation(selectedConversation.id, { silent: true });
    } catch (error) {
      Toast.error(`标记已读失败：${asError(error).message}`);
    } finally {
      if (mountedRef.current) {
        setConversationAction(null);
      }
    }
  }

  async function runStatusAction() {
    if (!selectedConversation || !canWrite || conversationAction) {
      return;
    }

    const nextStatus: SupportConversationStatus = selectedConversation.status === 'closed' ? 'open' : 'closed';
    setConversationAction('status');
    try {
      const result = await api.setStatus(selectedConversation.id, nextStatus);
      applyConversation(result);
      Toast.success(nextStatus === 'closed' ? '会话已关闭' : '会话已重新打开');
      void loadQueue({ silent: true });
      void loadConversation(selectedConversation.id, { silent: true });
    } catch (error) {
      Toast.error(`${nextStatus === 'closed' ? '关闭' : '重新打开'}会话失败：${asError(error).message}`);
    } finally {
      if (mountedRef.current) {
        setConversationAction(null);
      }
    }
  }

  async function sendReply(attempt: Omit<FailedReply, 'error'>) {
    if (replySending || !canWrite) {
      return;
    }

    setReplySending(true);
    setFailedReply(null);
    try {
      const result = await api.sendMessage(attempt.conversationId, {
        body: attempt.body,
        client_message_id: attempt.client_message_id
      });
      if (mountedRef.current) {
        applyConversation(result.conversation);
        setMessages((current) => mergeSupportMessages(current, [result.message]));
        setComposerDraft('');
        Toast.success('回复已发送');
        void loadQueue({ silent: true });
        void loadConversation(attempt.conversationId, { silent: true });
      }
    } catch (error) {
      if (mountedRef.current) {
        setFailedReply({ ...attempt, error: asError(error) });
      }
    } finally {
      if (mountedRef.current) {
        setReplySending(false);
      }
    }
  }

  function submitReply(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedConversation || selectedConversation.status === 'closed') {
      return;
    }

    const body = composerDraft.trim();
    if (!body) {
      Toast.warning('请输入回复内容');
      return;
    }
    if (scalarLength(body) > 2000) {
      Toast.warning('回复内容最多 2000 个字符');
      return;
    }

    const reusableAttempt = failedReply
      && failedReply.conversationId === selectedConversation.id
      && failedReply.body === body
      ? failedReply
      : null;
    void sendReply(reusableAttempt ?? {
      conversationId: selectedConversation.id,
      body,
      client_message_id: createSupportClientMessageId()
    });
  }

  const pageDescription = scope === 'admin'
    ? '查看全部客户会话并处理未分配队列；消息以服务端持久记录为准。'
    : '仅显示精确分配给当前代理的客户会话；上级代理不会自动看到下级代理会话。';

  return (
    <main className="exchange-page support-workbench-page">
      <PageHeader
        actions={
          <Button
            icon={<IconRefresh aria-hidden="true" />}
            loading={queueLoading}
            onClick={() => void loadQueue()}
          >
            立即同步
          </Button>
        }
        description={pageDescription}
        title="在线客服"
      />

      <Card bordered={false} className="support-queue-card" shadows="always">
        <div className="support-queue-toolbar">
          <div className="support-filter-section">
            <Text className="support-filter-label" strong>会话状态</Text>
            <div aria-label="会话状态筛选" className="support-filter-actions-row" role="group">
              {([
                ['open', '进行中'],
                ['closed', '已关闭'],
                ['all', '全部']
              ] as const).map(([value, label]) => (
                <Button
                  aria-pressed={statusFilter === value}
                  key={value}
                  onClick={() => {
                    setStatusFilter(value);
                    setQueuePage(1);
                  }}
                  theme={statusFilter === value ? 'solid' : 'light'}
                  type="primary"
                >
                  {label}
                </Button>
              ))}
            </div>
          </div>

          {scope === 'admin' ? (
            <div className="support-filter-section">
              <Text className="support-filter-label" strong>归属范围</Text>
              <div aria-label="会话归属筛选" className="support-filter-actions-row" role="group">
                {([
                  ['all', '全部客户'],
                  ['unassigned', '未分配客户']
                ] as const).map(([value, label]) => (
                  <Button
                    aria-pressed={assignmentFilter === value}
                    key={value}
                    onClick={() => {
                      setAssignmentFilter(value);
                      setQueuePage(1);
                    }}
                    theme={assignmentFilter === value ? 'solid' : 'light'}
                    type="tertiary"
                  >
                    {label}
                  </Button>
                ))}
              </div>
            </div>
          ) : null}

          <form className="support-search-form" onSubmit={submitKeyword}>
            <label className="support-search-label">
              <span>当前页关键词</span>
              <Input
                aria-label="搜索会话"
                onChange={(value) => setKeywordDraft(String(value))}
                placeholder="筛选当前页的用户、代理或消息"
                prefix={<IconSearch aria-hidden="true" />}
                showClear
                value={keywordDraft}
              />
            </label>
            <Button htmlType="submit" icon={<IconSearch aria-hidden="true" />} theme="solid" type="primary">
              搜索
            </Button>
          </form>
        </div>

        <div className="support-queue-summary">
          <Text type="tertiary">
            服务端共 {conversationTotal} 条，当前页筛选后显示 {visibleConversations.length} 条
          </Text>
          <Text type="tertiary">
            {lastReconciledAt ? <>最近同步：<TimestampText value={lastReconciledAt} /></> : '等待首次同步'}
          </Text>
        </div>

        {queueError && conversations.length > 0 ? (
          <Banner
            description={
              <Space>
                <span>自动同步失败：{queueError.message}</span>
                <Button onClick={() => void loadQueue()} size="small">重新加载</Button>
              </Space>
            }
            type="warning"
          />
        ) : null}

        <DataTable
          columns={queueColumns}
          data={visibleConversations}
          error={conversations.length === 0 ? queueError : null}
          loading={queueLoading}
          pagination={{
            currentPage: queuePage,
            onPageChange: setQueuePage,
            onPageSizeChange: (nextPageSize) => {
              setQueuePageSize(nextPageSize);
              setQueuePage(1);
            },
            pageSize: queuePageSize,
            total: conversationTotal
          }}
          rowKey="id"
        />
      </Card>

      <Card bordered={false} className="support-chat-card" shadows="always">
        {!selectedConversation ? (
          <div aria-live="polite" className="support-detail-state support-detail-empty" role="status">
            <Empty description="请选择一条会话" />
            <Text type="tertiary">打开队列中的会话后，可查看完整消息并处理客户问题。</Text>
          </div>
        ) : (
          <>
            <header className="support-conversation-header">
              <div className="support-conversation-title">
                <Title heading={4}>用户 {selectedConversation.user_id}</Title>
                <Space spacing={8} wrap>
                  {supportStatusTag(selectedConversation.status)}
                  <Tag color={selectedConversation.assigned_agent_id == null ? 'orange' : 'light-blue'}>
                    {supportAssignmentLabel(selectedConversation)}
                  </Tag>
                  {selectedConversation.staff_unread_count > 0 ? (
                    <Badge count={selectedConversation.staff_unread_count} overflowCount={99} type="danger">
                      <span className="support-unread-anchor">客服未读</span>
                    </Badge>
                  ) : null}
                </Space>
                <Text type="tertiary">联系方式：{contactLabel(selectedConversation)}</Text>
              </div>
              <Space className="support-conversation-actions" spacing={8} wrap>
                <Button
                  disabled={
                    !canWrite
                    || selectedConversation.staff_unread_count <= 0
                    || !(selectedConversation.last_message_id ?? sortedMessages.at(-1)?.id)
                    || conversationAction !== null
                  }
                  icon={<IconTickCircle aria-hidden="true" />}
                  loading={conversationAction === 'read'}
                  onClick={() => void runReadAction()}
                >
                  标记已读
                </Button>
                <Button
                  disabled={!canWrite || conversationAction !== null}
                  icon={selectedConversation.status === 'closed'
                    ? <IconUnlockStroked aria-hidden="true" />
                    : <IconLockStroked aria-hidden="true" />}
                  loading={conversationAction === 'status'}
                  onClick={() => void runStatusAction()}
                  type={selectedConversation.status === 'closed' ? 'primary' : 'danger'}
                >
                  {selectedConversation.status === 'closed' ? '重新打开' : '关闭会话'}
                </Button>
              </Space>
            </header>

            {!canWrite ? (
              <Banner description="当前角色只有会话读取权限，回复和状态操作已停用。" type="warning" />
            ) : null}

            <section aria-label="会话消息记录" className="support-message-history">
              {historyHasMore || olderMessagesLoading || olderMessagesError ? (
                <div aria-live="polite" className="support-history-pagination">
                  {olderMessagesError ? (
                    <Text role="alert" type="danger">
                      更早消息加载失败：{olderMessagesError.message}
                    </Text>
                  ) : null}
                  <Button
                    disabled={olderMessagesLoading || historyNextBeforeId === null}
                    loading={olderMessagesLoading}
                    onClick={() => void loadOlderMessages()}
                    size="small"
                  >
                    {olderMessagesError ? '重试加载更早消息' : '加载更早消息'}
                  </Button>
                </div>
              ) : null}
              {detailLoading && sortedMessages.length === 0 ? (
                <div aria-live="polite" className="support-detail-state" role="status">
                  <Spin size="large" tip="正在加载消息" />
                </div>
              ) : detailError && sortedMessages.length === 0 ? (
                <div className="support-detail-state" role="alert">
                  <Text type="danger">消息加载失败：{detailError.message}</Text>
                  <Button onClick={() => void loadConversation(selectedConversation.id)}>重新加载消息</Button>
                </div>
              ) : sortedMessages.length === 0 ? (
                <div aria-live="polite" className="support-detail-state" role="status">
                  <Empty description="暂无消息" />
                </div>
              ) : (
                <ol className="support-message-list">
                  {sortedMessages.map((message) => (
                    <li
                      className={isStaffMessage(message)
                        ? 'support-message support-message-staff'
                        : `support-message support-message-${message.sender_type}`}
                      key={message.id}
                    >
                      <div className="support-message-meta">
                        <Text strong>{messageSenderLabel(message)}</Text>
                        <TimestampText value={message.created_at} />
                      </div>
                      <p>{message.body}</p>
                    </li>
                  ))}
                </ol>
              )}
              {detailError && sortedMessages.length > 0 ? (
                <Banner
                  description={
                    <Space>
                      <span>消息同步失败：{detailError.message}</span>
                      <Button onClick={() => void loadConversation(selectedConversation.id)} size="small">重试同步</Button>
                    </Space>
                  }
                  type="warning"
                />
              ) : null}
            </section>

            {failedReply && failedReply.conversationId === selectedConversation.id ? (
              <Banner
                description={
                  <div className="support-reply-error">
                    <span>回复发送失败：{failedReply.error.message}</span>
                    <Text type="tertiary">原回复：{failedReply.body}</Text>
                    <Button
                      disabled={replySending}
                      loading={replySending}
                      onClick={() => void sendReply(failedReply)}
                      size="small"
                    >
                      重试发送
                    </Button>
                  </div>
                }
                type="danger"
              />
            ) : null}

            <form className="support-composer" onSubmit={submitReply}>
              <TextArea
                aria-label="回复内容"
                autosize={{ minRows: 3, maxRows: 7 }}
                disabled={!canWrite || selectedConversation.status === 'closed' || replySending}
                getValueLength={scalarLength}
                maxCount={2000}
                onChange={(value) => setComposerDraft(truncateScalars(value, 2000))}
                placeholder={selectedConversation.status === 'closed' ? '请先重新打开会话' : '输入回复内容'}
                showCounter
                value={composerDraft}
              />
              <div className="support-composer-footer">
                <Text type="tertiary">
                  {selectedConversation.status === 'closed'
                    ? '已关闭会话暂不可回复'
                    : `已输入 ${composerLength} / 2000 个字符`}
                </Text>
                <Button
                  disabled={!canWrite || selectedConversation.status === 'closed' || !composerDraft.trim() || replySending}
                  htmlType="submit"
                  icon={<IconSend aria-hidden="true" />}
                  loading={replySending}
                  theme="solid"
                  type="primary"
                >
                  发送回复
                </Button>
              </div>
            </form>
          </>
        )}
      </Card>
    </main>
  );
}
