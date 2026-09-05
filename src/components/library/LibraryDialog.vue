<script setup lang="ts">
import {
  DialogRoot,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
} from "reka-ui";
defineProps<{ title: string; busy?: boolean }>();
const emit = defineEmits<{ close: [] }>();
const returnFocus = document.activeElement;
function restoreFocus(event: Event): void {
  event.preventDefault();
  if (returnFocus instanceof HTMLElement && returnFocus.isConnected)
    returnFocus.focus();
}
</script>
<template>
  <DialogRoot :open="true" @update:open="!$event && !busy && emit('close')">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-50 bg-black/60" />
      <DialogContent
        aria-modal="true"
        :aria-describedby="undefined"
        class="library-dialog fixed top-1/2 left-1/2 z-50 flex max-h-[calc(100dvh-2rem)] w-[calc(100%-2rem)] max-w-130 -translate-x-1/2 -translate-y-1/2 flex-col overflow-y-auto rounded-xl border border-(--line-strong) bg-(--dialog-surface,var(--surface)) p-5 text-(--text)"
        @close-auto-focus="restoreFocus"
        @escape-key-down="busy && $event.preventDefault()"
        @interact-outside="busy && $event.preventDefault()"
      >
        <DialogTitle class="sr-only">{{ title }}</DialogTitle>
        <slot />
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
