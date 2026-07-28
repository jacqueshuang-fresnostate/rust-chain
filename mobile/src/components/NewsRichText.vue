<script setup lang="ts">
import type { NewsRichTextBlock, NewsRichTextLeaf } from '@/core/newsRichText'

defineProps<{
  blocks: NewsRichTextBlock[]
  emptyText: string
}>()

function leafClasses(leaf: NewsRichTextLeaf): Record<string, boolean> {
  return {
    'is-bold': leaf.bold,
    'is-italic': leaf.italic,
    'is-underline': leaf.underline,
  }
}
</script>

<template>
  <div class="news-rich-text">
    <template v-if="blocks.length">
      <template v-for="(block, blockIndex) in blocks" :key="blockIndex">
        <figure v-if="block.type === 'image'" class="news-rich-text__image">
          <img :src="block.url" :alt="block.alt" loading="lazy" decoding="async" />
          <figcaption v-if="block.alt">{{ block.alt }}</figcaption>
        </figure>
        <component :is="block.type" v-else>
          <component
            :is="leaf.href ? 'a' : 'span'"
            v-for="(leaf, leafIndex) in block.children"
            :key="leafIndex"
            :href="leaf.href"
            :target="leaf.href ? '_blank' : undefined"
            :rel="leaf.href ? 'noopener noreferrer' : undefined"
            :class="leafClasses(leaf)"
          >
            {{ leaf.text }}
          </component>
        </component>
      </template>
    </template>
    <p v-else>{{ emptyText }}</p>
  </div>
</template>

<style scoped>
.news-rich-text {
  color: var(--muted-strong);
  display: grid;
  font-size: 15px;
  gap: 14px;
  line-height: 1.85;
  margin-top: 22px;
  min-width: 0;
  overflow-wrap: anywhere;
}

.news-rich-text :where(p, h1, h2, h3, blockquote, figure) {
  margin: 0;
}

.news-rich-text h1 {
  color: var(--ink);
  font-size: 22px;
  line-height: 1.35;
}

.news-rich-text h2 {
  color: var(--ink);
  font-size: 19px;
  line-height: 1.4;
}

.news-rich-text h3 {
  color: var(--ink);
  font-size: 17px;
  line-height: 1.45;
}

.news-rich-text blockquote {
  border-left: 3px solid var(--accent);
  color: var(--muted);
  padding-left: 13px;
}

.news-rich-text a {
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 3px;
}

.news-rich-text .is-bold {
  font-weight: 800;
}

.news-rich-text .is-italic {
  font-style: italic;
}

.news-rich-text .is-underline {
  text-decoration: underline;
  text-underline-offset: 2px;
}

.news-rich-text__image {
  display: grid;
  gap: 6px;
}

.news-rich-text__image img {
  background: var(--soft);
  border: 1px solid var(--line);
  display: block;
  height: auto;
  max-height: 420px;
  object-fit: contain;
  width: 100%;
}

.news-rich-text__image figcaption {
  color: var(--muted);
  font-size: 11px;
  text-align: center;
}

@media (max-width: 340px) {
  .news-rich-text {
    font-size: 14px;
  }
}
</style>
