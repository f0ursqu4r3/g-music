<script setup lang="ts">
import { CheckCircle2, CircleAlert, LoaderCircle } from "lucide-vue-next";

import type { MetadataRefreshSnapshot } from "@/api";

interface Props {
  refreshes: MetadataRefreshSnapshot;
}

defineProps<Props>();

function stateLabel(state: string): string {
  const labels: Record<string, string> = {
    completed: "Refreshed",
    failed: "Retry on restart",
    queued: "Queued",
    refreshing: "Refreshing",
  };

  return labels[state] ?? state;
}
</script>

<template>
  <aside
    class="absolute top-0 right-0 z-20 flex max-h-full w-84 flex-col border-l border-(--line) bg-(--glass-panel) shadow-2xl backdrop-blur-xl"
    aria-label="Metadata refreshes"
  >
    <header class="border-b border-(--line) px-4 py-3">
      <p class="text-sm font-semibold">Metadata refresh</p>
      <p class="mt-0.5 text-xs text-(--muted-text)">
        {{ refreshes.completedTracks }} of {{ refreshes.totalTracks }} refreshed
      </p>
    </header>

    <ol class="min-h-0 overflow-auto p-2" aria-live="polite">
      <li
        v-for="job in refreshes.jobs"
        :key="`${job.trackId}-${job.state}`"
        class="rounded-lg px-3 py-2.5 text-sm"
        :data-refresh-track-id="job.trackId"
      >
        <div class="flex items-start gap-2">
          <LoaderCircle
            v-if="job.state === 'refreshing'"
            class="mt-0.5 size-4 shrink-0 animate-spin text-accent"
            aria-hidden="true"
          />
          <CheckCircle2
            v-else-if="job.state === 'completed'"
            class="mt-0.5 size-4 shrink-0 text-emerald-400"
            aria-hidden="true"
          />
          <CircleAlert
            v-else-if="job.state === 'failed'"
            class="mt-0.5 size-4 shrink-0 text-amber-300"
            aria-hidden="true"
          />
          <span
            v-else
            class="mt-1.5 size-2 shrink-0 rounded-full bg-(--subtle-text)"
            aria-hidden="true"
          />
          <div class="min-w-0">
            <p
              class="overflow-hidden font-medium text-ellipsis whitespace-nowrap"
            >
              {{ job.title }}
            </p>
            <p class="mt-0.5 text-xs text-(--muted-text)">
              {{ stateLabel(job.state) }} · {{ job.message }}
            </p>
          </div>
        </div>
      </li>
      <li
        v-if="refreshes.jobs.length === 0"
        class="px-3 py-5 text-center text-xs text-(--muted-text)"
      >
        No metadata refreshes are running.
      </li>
    </ol>
  </aside>
</template>
