<script setup lang="ts">
import type { ToasterProps } from "vue-sonner";
import {
  CircleCheckIcon,
  InfoIcon,
  Loader2Icon,
  OctagonXIcon,
  TriangleAlertIcon,
  XIcon,
} from "@lucide/vue";
import { reactiveOmit } from "@vueuse/core";
import { Toaster as Sonner } from "vue-sonner";
import "vue-sonner/style.css";
import { cn } from "@/lib/utils";

const props = withDefaults(defineProps<ToasterProps>(), { theme: "dark" });
const delegatedProps = reactiveOmit(props, "class", "toastOptions");
</script>

<template>
  <Sonner
    :class="cn('toaster group', props.class)"
    :toast-options="
      props.toastOptions ?? {
        classes: { toast: 'rounded-2xl', title: 'select-text break-words' },
      }
    "
    v-bind="delegatedProps"
  >
    <template #success-icon>
      <CircleCheckIcon class="size-4" />
    </template>
    <template #info-icon>
      <InfoIcon class="size-4" />
    </template>
    <template #warning-icon>
      <TriangleAlertIcon class="size-4" />
    </template>
    <template #error-icon>
      <OctagonXIcon class="size-4 text-(--error-text)" />
    </template>
    <template #loading-icon>
      <div><Loader2Icon class="size-4 animate-spin" /></div>
    </template>
    <template #close-icon>
      <XIcon class="size-4" />
    </template>
  </Sonner>
</template>

<style scoped>
:deep(.toaster[data-sonner-toaster]) {
  --normal-bg: var(--popover);
  --normal-text: var(--popover-foreground);
  --normal-border: var(--border);
  --border-radius: var(--radius);
  --gray2: color-mix(in oklch, var(--popover), transparent 10%);
  --gray3: var(--border);
  --gray4: var(--border);
  --gray5: var(--border);
  --gray12: var(--popover-foreground);
  width: min(var(--width), calc(100vw - 2rem));
  font-family: inherit;
}
</style>
