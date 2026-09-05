<script setup lang="ts">
import type { ContextMenuItemProps } from 'reka-ui'
import type { HTMLAttributes } from 'vue'
import { reactiveOmit } from '@vueuse/core'
import { ContextMenuItem, useForwardProps } from 'reka-ui'

import { cn } from '@/lib/utils'

const props = withDefaults(
  defineProps<
    ContextMenuItemProps & {
      class?: HTMLAttributes['class']
      variant?: 'default' | 'destructive'
    }
  >(),
  { variant: 'default' },
)

const delegatedProps = reactiveOmit(props, 'class', 'variant')
const forwardedProps = useForwardProps(delegatedProps)
</script>

<template>
  <ContextMenuItem
    data-slot="context-menu-item"
    :data-variant="variant"
    v-bind="forwardedProps"
    :class="
      cn(
        'flex cursor-default items-center rounded-none px-3.5 py-1.5 text-sm outline-none select-none data-highlighted:bg-(--text)/10 data-highlighted:text-(--text) data-disabled:pointer-events-none data-disabled:opacity-50 data-[variant=destructive]:text-red-300 data-[variant=destructive]:data-highlighted:bg-red-500/15',
        props.class,
      )
    "
  >
    <slot />
  </ContextMenuItem>
</template>
