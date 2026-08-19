import { client, requestUrl } from './client'

export type SupportConversationStatus = 'open' | 'closed'
export type SupportSenderType = 'user' | 'agent' | 'admin'

export interface Conversation {
  id: number
  user_id: number
  user_email: string | null
  user_phone: string | null
  assigned_agent_id: number | null
  assigned_agent_code: string | null
  status: SupportConversationStatus
  user_read_message_id: number | null
  staff_read_message_id: number | null
  user_unread_count: number
  staff_unread_count: number
  last_message_id: number | null
  last_message_sender_type: SupportSenderType | null
  last_message_sender_id: number | null
  last_message_preview: string | null
  last_message_at: number | null
  closed_at: number | null
  created_at: number
  updated_at: number
}

export interface Message {
  id: number
  conversation_id: number
  sender_type: SupportSenderType
  sender_id: number
  body: string
  client_message_id: string
  read_by_recipient: boolean
  created_at: number
}

export interface CurrentSupportConversationResponse {
  conversation: Conversation | null
}

export interface SupportMessagesResponse {
  messages: Message[]
  has_more: boolean
  next_before_id: number | null
}

export interface SendMessageInput {
  body: string
  clientMessageId: string
}

export interface SendMessageResult {
  conversation: Conversation
  message: Message
  replayed: boolean
}

export interface SupportMessagePagination {
  beforeId?: number
  limit?: number
}

export async function fetchCurrentSupportConversation(): Promise<CurrentSupportConversationResponse> {
  const response = await client.get<CurrentSupportConversationResponse>(requestUrl('/support/conversation'))
  return response.data
}

export async function fetchSupportConversationMessages(
  pagination: SupportMessagePagination = {},
): Promise<SupportMessagesResponse> {
  const response = await client.get<SupportMessagesResponse>(requestUrl('/support/conversation/messages'), {
    params: {
      before_id: pagination.beforeId,
      limit: pagination.limit,
    },
  })
  return response.data
}

export async function postSupportConversationMessage(input: SendMessageInput): Promise<SendMessageResult> {
  const response = await client.post<SendMessageResult>(requestUrl('/support/conversation/messages'), {
    body: input.body,
    client_message_id: input.clientMessageId,
  })
  return response.data
}

export async function markSupportConversationRead(messageId: number): Promise<Conversation> {
  const response = await client.post<Conversation>(requestUrl('/support/conversation/read'), {
    message_id: messageId,
  })
  return response.data
}

export async function patchSupportConversationStatus(
  status: SupportConversationStatus,
): Promise<Conversation> {
  const response = await client.patch<Conversation>(requestUrl('/support/conversation/status'), {
    status,
  })
  return response.data
}
