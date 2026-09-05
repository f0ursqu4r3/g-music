<script setup lang="ts" generic="T">
import {
  observeElementRect,
  type Rect,
  useVirtualizer,
  type Virtualizer,
} from '@tanstack/vue-virtual'
import { computed, ref } from 'vue'

import { ScrollArea } from '@/components/ui/scroll-area'

interface GridGroup<T> {
  items: T[]
  label: string
}

interface GridRow<T> {
  items: T[]
  key: string
  label: string
}

const props = withDefaults(
  defineProps<{
    getItemKey: (item: T) => string
    gridClass?: string
    gridItemSize: number
    groups: GridGroup<T>[]
    itemHeightPadding?: number
  }>(),
  {
    gridClass: '',
    itemHeightPadding: 48,
  },
)

defineSlots<{
  item(props: { item: T }): unknown
}>()

const horizontalPadding = 56
const gridGap = 16
const gridViewport = ref<HTMLElement | null>(null)
const viewportWidth = ref(defaultViewportWidth())

function defaultViewportHeight(): number {
  return typeof window === 'undefined' ? 600 : window.innerHeight || 600
}

function defaultViewportWidth(): number {
  return typeof window === 'undefined' ? 1024 : window.innerWidth || 1024
}

function updateViewportWidth(element: HTMLElement | null): void {
  viewportWidth.value = element?.clientWidth || defaultViewportWidth()
}

function setGridViewport(element: HTMLElement | null): void {
  gridViewport.value = element
  updateViewportWidth(element)
}

const columnCount = computed(() => {
  const contentWidth = Math.max(viewportWidth.value - horizontalPadding, 1)
  const minimumItemWidth = Math.min(props.gridItemSize, contentWidth)

  return Math.max(1, Math.floor((contentWidth + gridGap) / (minimumItemWidth + gridGap)))
})

const rows = computed<GridRow<T>[]>(() => {
  const nextRows: GridRow<T>[] = []

  props.groups.forEach((group, groupIndex) => {
    for (let itemIndex = 0; itemIndex < group.items.length; itemIndex += columnCount.value) {
      nextRows.push({
        items: group.items.slice(itemIndex, itemIndex + columnCount.value),
        key: `${groupIndex}-${itemIndex}`,
        label: itemIndex === 0 ? group.label : '',
      })
    }
  })

  return nextRows
})

function estimatedItemWidth(): number {
  const contentWidth = Math.max(viewportWidth.value - horizontalPadding, 1)
  return (contentWidth - gridGap * Math.max(columnCount.value - 1, 0)) / columnCount.value
}

function estimateRowHeight(index: number): number {
  const row = rows.value[index]
  const sectionHeight = row?.label ? 27 : 0
  return estimatedItemWidth() + props.itemHeightPadding + sectionHeight + gridGap
}

function observeGridRect(
  instance: Virtualizer<HTMLElement, Element>,
  callback: (rect: Rect) => void,
): (() => void) | undefined {
  return observeElementRect(instance, (rect) => {
    if (rect.width > 0) {
      viewportWidth.value = rect.width
    }
    callback({
      ...rect,
      height: rect.height > 0 ? rect.height : defaultViewportHeight(),
      width: rect.width > 0 ? rect.width : viewportWidth.value,
    })
  })
}

const virtualizerOptions = computed(() => {
  const scrollElement = gridViewport.value

  return {
    count: rows.value.length,
    estimateSize: estimateRowHeight,
    getScrollElement: () => scrollElement ?? gridViewport.value,
    initialRect: {
      height: defaultViewportHeight(),
      width: viewportWidth.value,
    },
    observeElementRect: observeGridRect,
    overscan: 3,
  }
})
const gridVirtualizer = useVirtualizer(virtualizerOptions)
const virtualRows = computed(() =>
  gridVirtualizer.value.getVirtualItems().flatMap((virtualItem) => {
    const row = rows.value[virtualItem.index]
    return row ? [{ row, virtualItem }] : []
  }),
)
const virtualGridHeight = computed(() => `${gridVirtualizer.value.getTotalSize()}px`)
</script>

<template>
  <ScrollArea class="size-full" type="scroll" :viewport-ref="setGridViewport">
    <div class="px-7 py-6">
      <div
        class="relative min-w-0"
        data-library-grid-virtualizer
        :style="{ height: virtualGridHeight }"
      >
        <section
          v-for="{ row, virtualItem } in virtualRows"
          :key="row.key"
          class="library-group absolute top-0 left-0 w-full"
          :data-index="virtualItem.index"
          :style="{ transform: `translateY(${virtualItem.start}px)` }"
        >
          <h2
            v-if="row.label"
            class="mb-3 text-2xl font-semibold tracking-wide text-(--muted-text) uppercase"
            data-library-section
          >
            {{ row.label }}
          </h2>
          <div class="library-grid" :class="props.gridClass">
            <div v-for="item in row.items" :key="props.getItemKey(item)" class="min-w-0">
              <slot name="item" :item="item" />
            </div>
          </div>
        </section>
      </div>
    </div>
  </ScrollArea>
</template>

<style scoped>
.library-grid {
  display: grid;
  grid-template-columns: repeat(
    auto-fill,
    minmax(min(v-bind('`${props.gridItemSize}px`'), 100%), 1fr)
  );
  align-content: start;
  gap: 1rem;
}
</style>
