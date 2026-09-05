<script setup lang="ts">
import { Check, ChevronRight, CircleAlert, CircleMinus, LoaderCircle, X } from 'lucide-vue-next'
import { computed } from 'vue'

import type { MetadataRefreshSnapshot } from '@/api'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'

interface Props {
  refreshes: MetadataRefreshSnapshot
  isRetrying?: boolean
  errorMessage?: string
}

const props = defineProps<Props>()
const emit = defineEmits<{ retry: []; close: [] }>()
const failedCount = computed(
  () => props.refreshes.jobs.filter((job) => job.state === 'failed').length,
)
const skippedCount = computed(
  () => props.refreshes.jobs.filter((job) => job.state === 'skipped').length,
)
const finishedCount = computed(() =>
  Math.min(props.refreshes.totalTracks, props.refreshes.completedTracks),
)
const refreshedCount = computed(() =>
  Math.max(0, finishedCount.value - failedCount.value - skippedCount.value),
)
const remainingCount = computed(() =>
  Math.max(0, props.refreshes.totalTracks - finishedCount.value),
)
const groups = computed(() =>
  [
    { state: 'refreshing', title: 'Refreshing now' },
    { state: 'failed', title: 'Needs attention' },
    { state: 'queued', title: 'Up next' },
    { state: 'completed', title: 'Refreshed' },
    { state: 'skipped', title: 'Skipped' },
  ]
    .map((group) => ({
      ...group,
      jobs: props.refreshes.jobs.filter((job) => job.state === group.state),
    }))
    .filter((group) => group.jobs.length),
)

function stateLabel(state: string): string {
  const labels: Record<string, string> = {
    completed: 'Refreshed',
    failed: 'Failed',
    queued: 'Queued',
    refreshing: 'Refreshing',
    skipped: 'Skipped',
  }

  return labels[state] ?? state
}
</script>

<template>
  <aside class="flex h-full min-h-0 flex-col bg-(--popover)" aria-label="Metadata refreshes">
    <header class="border-b border-(--line) px-4 py-3">
      <div class="flex items-center justify-between gap-2">
        <h2 class="text-sm font-semibold">Metadata refresh</h2>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          class="shrink-0"
          aria-label="Close metadata refresh"
          @click="emit('close')"
          ><X class="size-4" aria-hidden="true"
        /></Button>
      </div>
      <p class="mt-1 text-xs text-(--muted-text)" role="status">
        {{
          remainingCount
            ? `${remainingCount} tracks remaining`
            : failedCount
              ? 'Finished with errors'
              : skippedCount
                ? 'Finished with skipped tracks'
                : refreshes.totalTracks
                  ? 'All metadata refreshed'
                  : 'No refresh in progress'
        }}
      </p>
      <progress
        v-if="refreshes.totalTracks"
        :value="finishedCount"
        :max="refreshes.totalTracks"
        aria-label="Metadata refresh progress"
        :aria-valuetext="`${refreshedCount} refreshed, ${failedCount} failed${skippedCount ? `, ${skippedCount} skipped` : ''}, ${remainingCount} remaining`"
        class="mt-3 block h-1 w-full overflow-hidden rounded-full"
      />
      <p data-refresh-summary class="mt-2 text-xs text-(--muted-text)">
        {{ refreshedCount }} refreshed<span v-if="failedCount" class="text-(--warning)">
          · {{ failedCount }} failed</span
        ><span v-if="skippedCount" class="text-(--muted-text)"> · {{ skippedCount }} skipped</span>
        <span> · {{ refreshes.totalTracks }} total</span>
      </p>
      <Button
        v-if="failedCount"
        type="button"
        variant="outline"
        size="sm"
        aria-label="Retry failed metadata"
        class="mt-3"
        :disabled="isRetrying"
        @click="emit('retry')"
        >{{ isRetrying ? 'Retrying…' : 'Retry failed metadata' }}</Button
      >
      <p v-if="errorMessage" role="alert" class="window-alert-danger mt-2 break-words p-2 text-xs">
        {{ errorMessage }}
      </p>
    </header>

    <ScrollArea class="min-h-0 flex-1">
      <div class="px-4 py-2">
        <details
          v-for="group in groups"
          :key="group.state"
          :data-refresh-group="group.state"
          :open="group.state !== 'completed'"
          class="group border-b border-(--line) py-2 last:border-0"
        >
          <summary
            class="flex cursor-pointer list-none items-center gap-1.5 py-1 text-xs font-medium text-(--muted-text) outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring) [&::-webkit-details-marker]:hidden"
          >
            <ChevronRight
              class="size-3 transition-transform group-open:rotate-90 motion-reduce:transition-none"
              aria-hidden="true"
            />
            {{ group.title }}<span class="ml-auto tabular-nums">{{ group.jobs.length }}</span>
          </summary>
          <ol class="mt-1">
            <li
              v-for="job in group.jobs"
              :key="job.trackId"
              class="py-2 text-xs"
              :data-refresh-track-id="job.trackId"
            >
              <div class="flex items-start gap-2">
                <LoaderCircle
                  v-if="job.state === 'refreshing'"
                  class="size-3.5 shrink-0 animate-spin text-accent motion-reduce:animate-none"
                  aria-hidden="true"
                />
                <Check
                  v-else-if="job.state === 'completed'"
                  class="size-3.5 shrink-0 text-(--muted-text)"
                  aria-hidden="true"
                />
                <CircleAlert
                  v-else-if="job.state === 'failed'"
                  class="size-3.5 shrink-0 text-(--warning)"
                  aria-hidden="true"
                />
                <CircleMinus
                  v-else-if="job.state === 'skipped'"
                  class="size-3.5 shrink-0 text-(--muted-text)"
                  aria-hidden="true"
                />
                <span
                  v-else
                  class="mt-1 size-1.5 shrink-0 rounded-full bg-(--subtle-text)"
                  aria-hidden="true"
                />
                <div class="min-w-0 flex-1">
                  <p class="truncate font-medium" :title="job.title">
                    {{ job.title }}
                  </p>
                  <span class="sr-only">{{ stateLabel(job.state) }}</span>
                  <p
                    v-if="job.state === 'failed' || job.state === 'skipped'"
                    class="mt-1 break-words text-xs leading-5"
                    :class="job.state === 'failed' ? 'text-(--warning)' : 'text-(--muted-text)'"
                  >
                    {{ job.message }}
                  </p>
                </div>
              </div>
            </li>
          </ol>
        </details>
        <p v-if="!groups.length" class="py-4 text-xs text-(--muted-text)">
          No metadata refreshes are running.
        </p>
      </div>
    </ScrollArea>
    <p class="border-t border-(--line) px-4 py-3 text-xs text-(--muted-text)">
      Refresh continues when you close this panel.
    </p>
  </aside>
</template>

<style scoped>
progress {
  appearance: none;
  border: 0;
  background: var(--line);
}
progress::-webkit-progress-bar {
  background: var(--line);
}
progress::-webkit-progress-value {
  background: var(--accent);
}
progress::-moz-progress-bar {
  background: var(--accent);
}
</style>
