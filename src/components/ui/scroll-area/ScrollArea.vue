<script setup lang="ts">
import {
  ScrollAreaCorner,
  ScrollAreaRoot,
  ScrollAreaScrollbar,
  ScrollAreaThumb,
  ScrollAreaViewport,
} from 'reka-ui'
import { ref } from 'vue'
import type { ComponentPublicInstance, HTMLAttributes } from 'vue'

import { cn } from '@/lib/utils'

type ScrollOrientation = 'vertical' | 'horizontal' | 'both'
type ScrollAreaType = 'auto' | 'always' | 'hover' | 'scroll'

const props = withDefaults(
  defineProps<{
    class?: HTMLAttributes['class']
    orientation?: ScrollOrientation
    type?: ScrollAreaType
    viewportClass?: HTMLAttributes['class']
    viewportRef?: (element: HTMLElement | null) => void
  }>(),
  {
    orientation: 'vertical',
    type: 'auto',
  },
)

const viewportElement = ref<HTMLElement | null>(null)

function setRoot(element: Element | ComponentPublicInstance | null): void {
  const root =
    element instanceof HTMLElement
      ? element
      : element
        ? (element as ComponentPublicInstance).$el
        : null

  const viewport =
    root instanceof HTMLElement
      ? root.querySelector<HTMLElement>('[data-reka-scroll-area-viewport]')
      : null
  viewportElement.value = viewport
  props.viewportRef?.(viewportElement.value)
}
</script>

<template>
  <ScrollAreaRoot
    :ref="setRoot"
    data-slot="scroll-area"
    :type="props.type"
    :class="cn('relative min-h-0 min-w-0 overflow-hidden', props.class)"
  >
    <ScrollAreaViewport
      data-slot="scroll-area-viewport"
      :class="
        cn(
          'size-full rounded-[inherit] outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)',
          props.viewportClass,
        )
      "
    >
      <slot />
    </ScrollAreaViewport>
    <ScrollAreaScrollbar
      v-if="props.orientation === 'vertical' || props.orientation === 'both'"
      data-slot="scroll-area-scrollbar"
      orientation="vertical"
      class="flex touch-none select-none p-px transition-colors data-[orientation=vertical]:h-full data-[orientation=vertical]:w-2.5"
    >
      <ScrollAreaThumb
        data-slot="scroll-area-thumb"
        class="relative flex-1 rounded-full bg-(--line-strong) opacity-80 transition-colors hover:bg-(--muted-text)"
      />
    </ScrollAreaScrollbar>
    <ScrollAreaScrollbar
      v-if="props.orientation === 'horizontal' || props.orientation === 'both'"
      data-slot="scroll-area-scrollbar"
      orientation="horizontal"
      class="flex touch-none select-none p-px transition-colors data-[orientation=horizontal]:h-2.5 data-[orientation=horizontal]:flex-col"
    >
      <ScrollAreaThumb
        data-slot="scroll-area-thumb"
        class="relative flex-1 rounded-full bg-(--line-strong) opacity-80 transition-colors hover:bg-(--muted-text)"
      />
    </ScrollAreaScrollbar>
    <ScrollAreaCorner
      v-if="props.orientation === 'both'"
      data-slot="scroll-area-corner"
      class="bg-(--glass-window)"
    />
  </ScrollAreaRoot>
</template>
