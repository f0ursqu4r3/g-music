<script setup lang="ts">
import {
  CarFront,
  CassetteTape,
  CircleDot,
  Disc3,
  Heart,
  ListMusic,
  Mic2,
  Plus,
  Sparkles,
} from "lucide-vue-next";

import { ScrollArea } from "@/components/ui/scroll-area";
import type { LibraryCollection } from "./types";

const props = defineProps<{
  activeCollection: LibraryCollection;
}>();

const emit = defineEmits<{
  selectCollection: [collection: LibraryCollection];
  openImport: [];
}>();

const playlists = [
  { href: "#favorites", icon: Heart, label: "Favorites" },
  { href: "#chill-vibes", icon: Sparkles, label: "Chill Vibes" },
  { href: "#focus", icon: CircleDot, label: "Focus" },
  { href: "#road-trip", icon: CarFront, label: "Road Trip" },
  { href: "#90s-mix", icon: CassetteTape, label: "90s Mix" },
] as const;
</script>

<template>
  <aside
    class="col-start-1 row-start-1 mt-8 min-h-0 max-[760px]:hidden"
    data-library-sidebar
  >
    <ScrollArea class="size-full">
      <div class="p-4">
        <nav class="grid gap-0.5" aria-label="Library navigation">
          <div
            class="mb-1 flex items-center justify-between px-2.5"
            data-library-heading="library"
          >
            <p
              class="text-[0.61rem] font-semibold tracking-[0.06em] text-(--subtle-text)"
            >
              Library
            </p>
            <button
              aria-label="Import music"
              class="grid size-6 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4"
              type="button"
              @click="emit('openImport')"
            >
              <Plus aria-hidden="true" />
            </button>
          </div>
          <button
            v-for="collection in [
              ['tracks', ListMusic, 'Tracks'],
              ['albums', Disc3, 'Albums'],
              ['artists', Mic2, 'Artists'],
            ] as const"
            :key="collection[0]"
            :aria-current="
              props.activeCollection === collection[0] ? 'page' : undefined
            "
            class="flex min-h-8.5 w-full cursor-pointer items-center gap-2.5 rounded-md border-0 bg-transparent px-2.5 text-left text-[0.82rem] text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.7_0.03_262/0.15)] aria-[current=page]:text-(--text) aria-[current=page]:[&>svg]:text-accent [&>svg]:size-4"
            :data-collection="collection[0]"
            type="button"
            @click="emit('selectCollection', collection[0])"
          >
            <component :is="collection[1]" aria-hidden="true" />{{
              collection[2]
            }}
          </button>
          <span
            class="flex min-h-8.5 items-center gap-2.5 rounded-md px-2.5 text-[0.82rem] text-(--muted-text) [&>svg]:size-4"
            data-library-destination="playlists"
          >
            <ListMusic aria-hidden="true" />Playlists
          </span>
        </nav>

        <nav
          class="mt-5 grid gap-0.5"
          aria-label="Playlists"
          data-library-playlists
        >
          <div class="mb-1 flex items-center justify-between px-2.5">
            <p
              class="text-[0.61rem] font-semibold tracking-[0.06em] text-(--subtle-text)"
            >
              Playlists
            </p>
            <button
              aria-label="New playlist"
              class="grid size-6 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4"
              type="button"
            >
              <Plus aria-hidden="true" />
            </button>
          </div>
          <a
            v-for="playlist in playlists"
            :key="playlist.href"
            class="flex min-h-8 items-center gap-2.5 rounded-md px-2.5 text-[0.79rem] text-(--muted-text) no-underline transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4"
            :href="playlist.href"
          >
            <component :is="playlist.icon" aria-hidden="true" />
            {{ playlist.label }}
          </a>
        </nav>
      </div>
    </ScrollArea>
  </aside>
</template>

<style scoped>
/* The sidebar has no component-specific rules beyond utility classes. */
</style>
