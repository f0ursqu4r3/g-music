<script setup lang="ts">
import {
  CircleAlert,
  ExternalLink,
  Link2,
  LogOut,
  Youtube,
} from "lucide-vue-next";
import { onMounted, ref } from "vue";

import { youtubeAuthApi, type YouTubeAuthStatus } from "@/api";
import { Button } from "@/components/ui/button";

const status = ref<YouTubeAuthStatus>({ connected: false });
const isBusy = ref(false);
const isReady = ref(false);
const errorMessage = ref("");

function readError(error: unknown): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return "The YouTube account session could not be updated.";
}

async function updateStatus(
  operation: () => Promise<YouTubeAuthStatus>,
): Promise<void> {
  isBusy.value = true;
  errorMessage.value = "";
  try {
    status.value = await operation();
  } catch (error) {
    errorMessage.value = readError(error);
  } finally {
    isBusy.value = false;
    isReady.value = true;
  }
}

function openLogin(): Promise<void> {
  return updateStatus(youtubeAuthApi.openLogin);
}

function saveSession(): Promise<void> {
  return updateStatus(youtubeAuthApi.saveSession);
}

function disconnect(): Promise<void> {
  return updateStatus(youtubeAuthApi.disconnect);
}

onMounted(() => {
  void updateStatus(youtubeAuthApi.inspect);
});
</script>

<template>
  <main
    aria-label="Settings"
    class="window-shell window-surface px-8 pt-6 pb-10"
  >
    <div class="mx-auto grid max-w-2xl gap-6">
      <header class="grid gap-1">
        <p class="window-kicker m-0 tracking-[0.14em]">Accounts</p>
        <h1 class="window-title m-0 tracking-[-0.025em]">YouTube playback</h1>
        <p class="window-copy m-0 max-w-xl text-sm leading-6">
          Connect a dedicated YouTube session for age-restricted, private, and
          account-only audio.
        </p>
      </header>

      <section
        class="window-panel overflow-hidden rounded-2xl shadow-sm"
        aria-labelledby="youtube-account-heading"
      >
        <div class="flex items-start gap-4 p-5">
          <div
            class="grid size-11 shrink-0 place-items-center rounded-xl bg-red-500/12 text-red-500"
          >
            <Youtube class="size-6" aria-hidden="true" />
          </div>

          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div>
                <h2
                  id="youtube-account-heading"
                  class="m-0 text-base font-semibold"
                >
                  YouTube account
                </h2>
                <p
                  :data-auth-state="
                    status.connected ? 'connected' : 'disconnected'
                  "
                  class="m-0 mt-1 text-sm text-(--muted-text)"
                >
                  <template v-if="!isReady">Checking session…</template>
                  <template v-else-if="status.connected">
                    Connected with a dedicated local session
                  </template>
                  <template v-else>Not connected</template>
                </p>
              </div>

              <Button
                v-if="status.connected"
                type="button"
                variant="outline"
                aria-label="Disconnect YouTube"
                :disabled="isBusy"
                @click="disconnect"
              >
                <LogOut data-icon="inline-start" />
                Disconnect
              </Button>
            </div>
          </div>
        </div>

        <div
          v-if="!status.connected"
          class="grid gap-4 border-t border-(--line) p-5"
        >
          <ol class="m-0 grid list-none gap-3 p-0 text-sm text-(--muted-text)">
            <li class="flex items-start gap-3">
              <span
                class="grid size-5 shrink-0 place-items-center rounded-full bg-(--surface-muted) text-[0.68rem] font-bold text-(--text-primary)"
                >1</span
              >
              <span>Open the isolated Google sign-in window.</span>
            </li>
            <li class="flex items-start gap-3">
              <span
                class="grid size-5 shrink-0 place-items-center rounded-full bg-(--surface-muted) text-[0.68rem] font-bold text-(--text-primary)"
                >2</span
              >
              <span
                >Complete sign-in, then return here and save the session.</span
              >
            </li>
          </ol>

          <div class="flex flex-wrap gap-2">
            <Button
              type="button"
              aria-label="Sign in to YouTube"
              :disabled="isBusy"
              @click="openLogin"
            >
              <ExternalLink data-icon="inline-start" />
              Open YouTube sign-in
            </Button>
            <Button
              type="button"
              variant="outline"
              aria-label="Use signed-in session"
              :disabled="isBusy"
              @click="saveSession"
            >
              <Link2 data-icon="inline-start" />
              Use signed-in session
            </Button>
          </div>
        </div>
      </section>

      <aside
        class="window-alert-warning flex items-start gap-3 p-4 text-sm leading-5"
      >
        <CircleAlert
          class="mt-0.5 size-4 shrink-0 text-amber-500"
          aria-hidden="true"
        />
        <p class="m-0">
          YouTube does not officially support yt-dlp sessions. Your account can
          be restricted or banned. Use a secondary account and disconnect it
          when you no longer need authenticated playback.
        </p>
      </aside>

      <p
        v-if="errorMessage"
        class="window-alert-danger m-0 px-4 py-3 text-sm"
        role="alert"
      >
        {{ errorMessage }}
      </p>
    </div>
  </main>
</template>
