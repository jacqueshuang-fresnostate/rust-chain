import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { normalizeNewsRichText, selectLocalizedNewsRichText } from '../src/core/newsRichText.ts'

test('localized news rich text preserves safe marks, links, headings, quotes, and images', () => {
  const blocks = selectLocalizedNewsRichText({
    default_locale: 'en-US',
    items: [
      { locale: 'en-US', content: [{ type: 'p', children: [{ text: 'English' }] }] },
      {
        locale: 'zh-CN',
        content: [
          {
            type: 'h2',
            children: [
              { text: '安全公告', bold: true, italic: true, underline: true },
              { text: '查看详情', link: 'https://example.test/news/1' },
            ],
          },
          { type: 'blockquote', children: [{ text: '风险提示' }] },
          { type: 'image', url: 'https://cdn.example.test/news/body.png', alt: '公告配图' },
        ],
      },
    ],
  }, 'zh-CN')

  assert.deepEqual(blocks, [
    {
      type: 'h2',
      children: [
        { text: '安全公告', bold: true, italic: true, underline: true, href: undefined },
        { text: '查看详情', bold: false, italic: false, underline: false, href: 'https://example.test/news/1' },
      ],
    },
    {
      type: 'blockquote',
      children: [{ text: '风险提示', bold: false, italic: false, underline: false, href: undefined }],
    },
    { type: 'image', url: 'https://cdn.example.test/news/body.png', alt: '公告配图' },
  ])
})

test('news rich text rejects executable links and keeps legacy strings as text', () => {
  assert.deepEqual(normalizeNewsRichText('<b>legacy text</b>'), [{
    type: 'p',
    children: [{ text: '<b>legacy text</b>', bold: false, italic: false, underline: false, href: undefined }],
  }])
  assert.deepEqual(normalizeNewsRichText([{
    type: 'p',
    children: [{ text: 'unsafe', href: 'javascript:alert(1)' }],
  }]), [{
    type: 'p',
    children: [{ text: 'unsafe', bold: false, italic: false, underline: false, href: undefined }],
  }])
})

test('news detail uses a structured Vue renderer and never injects backend HTML', async () => {
  const viewSource = await readFile(new URL('../src/views/NewsDetailView.vue', import.meta.url), 'utf8')
  const rendererSource = await readFile(new URL('../src/components/NewsRichText.vue', import.meta.url), 'utf8')
  assert.match(viewSource, /<NewsRichText :blocks="detail\.content"/)
  assert.match(rendererSource, /block\.type === 'image'/)
  assert.match(rendererSource, /noopener noreferrer/)
  assert.doesNotMatch(viewSource, /v-html/)
  assert.doesNotMatch(rendererSource, /v-html/)
})
