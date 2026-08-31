import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileStyle } from 'vue/compiler-sfc'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import { filterDocumentTypeOptions } from '../src/core/kycDocumentSearch.ts'

const source = readFileSync(new URL('../src/views/KycView.vue', import.meta.url), 'utf8')

const documentTypes = [
  { value: 'identity_card', label: '身份证' },
  { value: 'passport', label: '护照' },
  { value: 'résidence-permit', label: '居住证' },
  { value: 'custom_document', label: 'Custom credential' },
]

test('KYC 证件类型搜索匹配本地化名称和后台原始值并保持配置顺序', () => {
  assert.deepEqual(filterDocumentTypeOptions(documentTypes, '身份证'), [documentTypes[0]])
  assert.deepEqual(filterDocumentTypeOptions(documentTypes, 'identity card'), [documentTypes[0]])
  assert.deepEqual(filterDocumentTypeOptions(documentTypes, 'Residence permit'), [documentTypes[2]])
  assert.deepEqual(filterDocumentTypeOptions(documentTypes, 'custom cred'), [documentTypes[3]])
  assert.deepEqual(filterDocumentTypeOptions(documentTypes, '  '), documentTypes)
  assert.deepEqual(filterDocumentTypeOptions(documentTypes, '不存在'), [])
})

test('KYC 证件类型使用独立可搜索 Teleport 弹层并保留后台原始值', () => {
  assert.match(source, /useModalDialog\(documentTypePickerOpen, documentTypePickerDialog, '\[data-document-type-search\]'\)/)
  assert.match(source, /setDocumentTypePickerReturnFocus\(documentTypePickerTrigger\.value\)/)
  assert.match(source, /filterDocumentTypeOptions\(\s*documentTypeOptions\.value,\s*documentTypeSearch\.value,?\s*\)/)
  assert.match(source, /form\.value\.documentType = option\.value/)
  assert.match(source, /trapDocumentTypePickerFocus\(event, closeDocumentTypePicker\)/)
  assert.match(source, /id="kyc-document-type-picker-trigger"[\s\S]*?aria-haspopup="dialog"[\s\S]*?:aria-expanded="documentTypePickerOpen"/)
  assert.match(source, /id="kyc-document-type-picker"[\s\S]*?role="dialog"[\s\S]*?aria-modal="true"/)
  assert.match(source, /data-document-type-search[\s\S]*?type="search"/)
  assert.match(source, /v-for="option in filteredDocumentTypeOptions"[\s\S]*?:aria-pressed="isDocumentTypeSelected\(option\.value\)"[\s\S]*?@click="selectDocumentType\(option\.value\)"/)
  assert.match(source, /v-if="!filteredDocumentTypeOptions\.length"[\s\S]*?kyc\.documentTypeNoResults/)
  assert.doesNotMatch(source, /<select v-model="form\.documentType"/)
})

test('KYC 国家与证件类型搜索弹层共享主题和受限设备降级样式', () => {
  assert.match(source, /class="kyc-picker-mask kyc-country-picker-mask"/)
  assert.match(source, /class="kyc-picker-mask kyc-document-picker-mask"/)
  assert.match(source, /\.kyc-picker-search \{[^}]*background: color-mix\(in srgb, var\(--surface-elevated\) 90%, var\(--ink\)\)/)
  assert.match(source, /:global\(html\[data-performance-tier='constrained'\] \.kyc-picker-mask\)/)

  const scopedStyle = source.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1] || ''
  const compiled = compileStyle({
    source: scopedStyle,
    filename: 'KycView.vue',
    id: 'data-v-kyc-document-search',
    scoped: true,
  })

  assert.deepEqual(compiled.errors, [])
  assert.match(compiled.code, /html\[data-performance-tier=['"]?constrained['"]?\] \.kyc-picker-mask\s*\{[^}]*backdrop-filter:\s*none/)
})

test('KYC 证件类型搜索文案在中英文完整存在', () => {
  for (const messages of [zhCN, en]) {
    assert.ok(messages.kyc.selectDocumentType)
    assert.ok(messages.kyc.documentTypePickerTitle)
    assert.ok(messages.kyc.documentTypePickerClose)
    assert.ok(messages.kyc.documentTypeSearchLabel)
    assert.ok(messages.kyc.documentTypeSearchPlaceholder)
    assert.ok(messages.kyc.documentTypeNoResults)
  }
})
