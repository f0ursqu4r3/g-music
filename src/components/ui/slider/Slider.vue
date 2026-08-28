<script setup lang="ts">
import { reactiveOmit } from "@vueuse/core";
import {
  SliderRange,
  SliderRoot,
  SliderThumb,
  SliderTrack,
  useForwardPropsEmits,
} from "reka-ui";
import type { SliderRootEmits, SliderRootProps } from "reka-ui";
import { computed, useAttrs } from "vue";
import type { HTMLAttributes } from "vue";

import { cn } from "@/lib/utils";

const props = defineProps<
  SliderRootProps & { class?: HTMLAttributes["class"] }
>();
const emits = defineEmits<SliderRootEmits>();
const delegatedProps = reactiveOmit(props, "class");
const forwarded = useForwardPropsEmits(delegatedProps, emits);
const attrs = useAttrs();
const thumbLabel = computed(() => {
  const label = attrs["aria-label"];
  return typeof label === "string" ? label : undefined;
});
</script>

<template>
  <SliderRoot
    data-slot="slider"
    v-bind="forwarded"
    :class="
      cn(
        'group/slider relative flex w-full touch-none items-center select-none data-[disabled]:opacity-50 data-[orientation=vertical]:h-full data-[orientation=vertical]:min-h-44 data-[orientation=vertical]:w-auto data-[orientation=vertical]:flex-col',
        props.class,
      )
    "
  >
    <SliderTrack
      data-slot="slider-track"
      class="relative grow overflow-hidden rounded-full bg-(--line-strong) data-[orientation=horizontal]:h-1 data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-1"
    >
      <SliderRange
        data-slot="slider-range"
        class="absolute bg-accent data-[orientation=horizontal]:h-full data-[orientation=vertical]:w-full"
      />
    </SliderTrack>
    <SliderThumb
      v-for="(_, index) in modelValue ?? defaultValue ?? [0]"
      :key="index"
      data-slot="slider-thumb"
      :aria-label="thumbLabel"
      class="block size-3.5 shrink-0 rounded-full border border-accent bg-(--text) opacity-0 shadow-sm ring-(--focus-ring)/50 transition-[opacity,box-shadow] duration-150 group-hover/slider:opacity-100 group-focus-within/slider:opacity-100 hover:ring-4 focus-visible:ring-4 focus-visible:outline-hidden disabled:pointer-events-none"
    />
  </SliderRoot>
</template>
