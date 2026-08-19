import { apiRequest } from './client';

export type StaffSupportScope = 'agent' | 'admin';
export type SupportConversationStatus = 'open' | 'closed';
export type SupportSenderType = 'user' | 'agent' | 'admin';

export type SupportConversation = {
  id: number;
  user_id: number;
  email: string | null;
  phone: string | null;
  assigned_agent_id: number | null;
  assigned_agent_code: string | null;
  status: SupportConversationStatus;
  user_read_message_id: number | null;
  staff_read_message_id: number | null;
  user_unread_count: number;
  staff_unread_count: number;
  last_message_id: number | null;
  last_message_sender_type: SupportSenderType | null;
  last_message_sender_id: number | null;
  last_message_preview: string | null;
  last_message_at: number | null;
  closed_at: number | null;
  created_at: number;
  updated_at: number;
};

export type SupportMessage = {
  id: number;
  conversation_id: number;
  sender_type: SupportSenderType;
  sender_id: number;
  body: string;
  client_message_id: string;
  read_by_recipient: boolean;
  created_at: number;
};

export type SupportConversationList = {
  conversations: SupportConversation[];
  total: number;
};

export type SupportSendMessageResponse = {
  conversation: SupportConversation;
  message: SupportMessage;
  replayed: boolean;
};

export type SupportMessages = {
  has_more: boolean;
  messages: SupportMessage[];
  next_before_id: number | null;
};

export type SupportConversationFilters = {
  assigned_agent_id?: number;
  limit?: number;
  offset?: number;
  status?: SupportConversationStatus;
  unassigned?: boolean;
  unread_only?: boolean;
};

export type SupportMessagePagination = {
  before_id?: number;
  limit?: number;
};

export type SupportReplyInput = {
  body: string;
  client_message_id: string;
};

export type StaffSupportApi = {
  getConversation: (conversationId: number) => Promise<SupportConversation>;
  getMessages: (conversationId: number, pagination?: SupportMessagePagination) => Promise<SupportMessages>;
  listConversations: (filters?: SupportConversationFilters) => Promise<SupportConversationList>;
  markRead: (conversationId: number, messageId: number) => Promise<SupportConversation>;
  sendMessage: (conversationId: number, input: SupportReplyInput) => Promise<SupportSendMessageResponse>;
  setStatus: (
    conversationId: number,
    status: SupportConversationStatus
  ) => Promise<SupportConversation>;
};

const supportScopes: Record<StaffSupportScope, { authScope: StaffSupportScope; prefix: string }> = {
  agent: { authScope: 'agent', prefix: '/agent/api/v1/support' },
  admin: { authScope: 'admin', prefix: '/admin/api/v1/support' }
};

type SupportConversationWire = Omit<SupportConversation, 'email' | 'phone'> & {
  user_email: string | null;
  user_phone: string | null;
};

type SupportSendMessageResponseWire = Omit<SupportSendMessageResponse, 'conversation'> & {
  conversation: SupportConversationWire;
};

function appendQuery(path: string, values: Record<string, string | number | undefined>): string {
  const query = new URLSearchParams();
  Object.entries(values).forEach(([key, value]) => {
    if (value !== undefined && value !== '') {
      query.set(key, String(value));
    }
  });

  const serialized = query.toString();
  return serialized ? `${path}?${serialized}` : path;
}

function normalizeConversation(conversation: SupportConversationWire): SupportConversation {
  const { user_email, user_phone, ...fields } = conversation;
  return {
    ...fields,
    email: user_email,
    phone: user_phone
  };
}

/**
 * 代理端与管理端共享同一 DTO 和动作集合，但前缀与认证作用域在这里一次性绑定。
 * 页面只接收本客户端，避免代理页面误用管理端默认作用域或拼出跨身份路径。
 */
export function createStaffSupportApi(scope: StaffSupportScope): StaffSupportApi {
  const { authScope, prefix } = supportScopes[scope];
  const request = <T>(path: string, init: RequestInit = {}) =>
    apiRequest<T>(`${prefix}${path}`, { ...init, authScope });

  return {
    async listConversations(filters = {}) {
      const response = await request<{ conversations: SupportConversationWire[]; total: number }>(
        appendQuery('/conversations', {
          status: filters.status,
          unread_only: filters.unread_only ? 'true' : undefined,
          assigned_agent_id: filters.assigned_agent_id,
          unassigned: filters.unassigned ? 'true' : undefined,
          limit: filters.limit,
          offset: filters.offset
        })
      );
      return {
        conversations: response.conversations.map(normalizeConversation),
        total: response.total
      };
    },

    async getConversation(conversationId) {
      const response = await request<SupportConversationWire>(
        `/conversations/${conversationId}`
      );
      return normalizeConversation(response);
    },

    getMessages(conversationId, pagination = {}) {
      return request<SupportMessages>(
        appendQuery(`/conversations/${conversationId}/messages`, {
          limit: pagination.limit,
          before_id: pagination.before_id
        })
      );
    },

    async sendMessage(conversationId, input) {
      const response = await request<SupportSendMessageResponseWire>(`/conversations/${conversationId}/messages`, {
        method: 'POST',
        body: JSON.stringify(input)
      });
      return {
        ...response,
        conversation: normalizeConversation(response.conversation)
      };
    },

    async markRead(conversationId, messageId) {
      const response = await request<SupportConversationWire>(`/conversations/${conversationId}/read`, {
        method: 'POST',
        body: JSON.stringify({ message_id: messageId })
      });
      return normalizeConversation(response);
    },

    async setStatus(conversationId, status) {
      const response = await request<SupportConversationWire>(`/conversations/${conversationId}/status`, {
        method: 'PATCH',
        body: JSON.stringify({ status })
      });
      return normalizeConversation(response);
    }
  };
}
