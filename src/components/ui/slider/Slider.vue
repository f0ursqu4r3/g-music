<script setup lang="ts">
import { reactiveOmit } from "@vueuse/core";
import {
  SliderRange,
  SliderRoot,
  SliderThumb,
  SliderTrack,
  useForwardProps,
} from "reka-ui";
import type { SliderRootEmits, SliderRootProps } from "reka-ui";
import { computed, ref, useAttrs, watch } from "vue";
import type { HTMLAttributes } from "vue";

import { cn } from "@/lib/utils";

const props = defineProps<
  SliderRootProps & { class?: HTMLAttributes["class"] }
>();
const emits = defineEmits<SliderRootEmits>();
const delegatedProps = reactiveOmit(
  props,
  "class",
  "defaultValue",
  "modelValue",
);
const forwardedProps = useForwardProps(delegatedProps);
const attrs = useAttrs();
const isInteracting = ref(false);
const internalModelValue = ref([
  ...(props.modelValue ?? props.defaultValue ?? [0]),
]);
const thumbLabel = computed(() => {
  const label = attrs["aria-label"];
  return typeof label === "string" ? label : undefined;
});

watch(
  () => props.modelValue,
  (modelValue) => {
    if (!isInteracting.value && modelValue) {
      internalModelValue.value = [...modelValue];
    }
  },
);

function updateModelValue(modelValue: number[] | undefined): void {
  if (!modelValue) {
    return;
  }

  internalModelValue.value = [...modelValue];
  emits("update:modelValue", modelValue);
}

function stopInteraction(): void {
  isInteracting.value = false;
}
</script>

<template>
  <SliderRoot
    data-slot="slider"
    v-bind="forwardedProps"
    :model-value="internalModelValue"
    :class="
      cn(
        'group/slider relative flex w-full touch-none items-center select-none data-[disabled]:opacity-50 data-[orientation=vertical]:h-full data-[orientation=vertical]:min-h-44 data-[orientation=vertical]:w-auto data-[orientation=vertical]:flex-col',
        props.class,
      )
    "
    @pointerdown.capture="isInteracting = true"
    @pointercancel="stopInteraction"
    @pointerup="stopInteraction"
    @update:model-value="updateModelValue"
    @value-commit="emits('valueCommit', $event)"
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
      v-for="(_, index) in internalModelValue"
      :key="index"
      data-slot="slider-thumb"
      :aria-label="thumbLabel"
      class="block size-3.5 shrink-0 rounded-full border border-accent bg-(--text) opacity-0 shadow-sm ring-(--focus-ring)/50 transition-[opacity,box-shadow] duration-150 group-hover/slider:opacity-100 group-focus-within/slider:opacity-100 hover:ring-4 focus-visible:ring-4 focus-visible:outline-hidden disabled:pointer-events-none"
    />
  </SliderRoot>
</template>
