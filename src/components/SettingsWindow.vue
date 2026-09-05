<script setup lang="ts">
import {
  CircleAlert,
  ExternalLink,
  Link2,
  LogOut,
  Youtube,
} from "lucide-vue-next";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

import {
  playbackApi,
  youtubeAuthApi,
  type DiagnosticsSnapshot,
  type YouTubeAuthStatus,
} from "@/api";
import { Button } from "@/components/ui/button";
import { readTheme, themes, type ThemeName } from "@/lib/theme";

defineProps<{ theme?: ThemeName; playbackError?: string }>();
const emit = defineEmits<{ "update:theme": [theme: ThemeName]; retry: [] }>();

const status = ref<YouTubeAuthStatus>({ connected: false });
const isBusy = ref(false);
const isReady = ref(false);
const errorMessage = ref("");
const diagnostics = ref<DiagnosticsSnapshot | null>(null);
const diagnosticsError = ref("");
const isInspecting = ref(false);
const isBackingUp = ref(false);
const backupPath = ref("");
const backupError = ref("");
const copyMessage = ref("");
const copyError = ref("");
const isPlaybackReady = computed(() =>
  diagnostics.value?.dependencies.every((dependency) => dependency.available),
);
let isAlive = true;
onBeforeUnmount(() => {
  isAlive = false;
});

async function refreshDiagnostics(): Promise<void> {
  if (isInspecting.value) return;
  isInspecting.value = true;
  diagnosticsError.value = "";
  try {
    const result = await playbackApi.inspectDiagnostics();
    if (isAlive) diagnostics.value = result;
  } catch (error) {
    if (isAlive)
      diagnosticsError.value = readError(
        error,
        "Diagnostics could not be loaded. Refresh diagnostics to retry.",
      );
  } finally {
    if (isAlive) isInspecting.value = false;
  }
}

async function backUpLibrary(): Promise<void> {
  if (isBackingUp.value) return;
  isBackingUp.value = true;
  backupError.value = "";
  try {
    const result = await playbackApi.exportLibraryBackup();
    if (isAlive) backupPath.value = result.path;
  } catch (error) {
    if (isAlive)
      backupError.value = readError(
        error,
        "Backup failed. Check free disk space and retry.",
      );
  } finally {
    if (isAlive) isBackingUp.value = false;
  }
}

async function copyText(value: string, success: string): Promise<void> {
  copyMessage.value = "";
  copyError.value = "";
  try {
    await navigator.clipboard.writeText(value);
    if (isAlive) copyMessage.value = success;
  } catch (error) {
    if (isAlive)
      copyError.value = readError(
        error,
        "Copy failed. Select the text and copy it manually.",
      );
  }
}

function setTheme(event: Event): void {
  emit("update:theme", readTheme((event.target as HTMLSelectElement).value));
}

function readError(
  error: unknown,
  fallback = "The YouTube account session could not be updated.",
): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return typeof error === "string" ? error : fallback;
}

async function updateStatus(
  operation: () => Promise<YouTubeAuthStatus>,
): Promise<void> {
  isBusy.value = true;
  errorMessage.value = "";
  try {
    const result = await operation();
    if (isAlive) status.value = result;
  } catch (error) {
    if (isAlive) errorMessage.value = readError(error);
  } finally {
    if (isAlive) {
      isBusy.value = false;
      isReady.value = true;
    }
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
  void refreshDiagnostics();
});
</script>

<template>
  <main
    aria-label="Settings"
    class="window-shell window-surface h-screen overflow-y-auto px-8 pt-6 pb-10"
  >
    <div class="mx-auto grid max-w-2xl gap-6">
      <header class="grid gap-1">
        <h1 class="window-title m-0 tracking-[-0.025em]">Settings</h1>
        <p class="window-copy m-0 max-w-xl text-sm leading-6">
          Manage appearance, playback readiness, library backups, and your local
          YouTube session.
        </p>
      </header>

      <section class="window-panel p-5" aria-labelledby="appearance-heading">
        <h2 id="appearance-heading" class="text-base font-semibold">
          Appearance
        </h2>
        <label for="settings-theme" class="mt-3 block text-sm">Theme</label>
        <select
          id="settings-theme"
          aria-label="Theme"
          :value="theme ?? 'midnight'"
          class="mt-2 rounded-lg border border-(--line-strong) bg-(--glass-control) px-3 py-2 text-sm text-(--text) outline-none focus:border-(--focus-ring)"
          @change="setTheme"
        >
          <option v-for="name in themes" :key="name" :value="name">
            {{ name[0]?.toUpperCase() }}{{ name.slice(1) }}
          </option>
        </select>
      </section>

      <section class="window-panel p-5" aria-labelledby="diagnostics-heading">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <h2 id="diagnostics-heading" class="text-base font-semibold">
            Playback readiness
          </h2>
          <Button
            type="button"
            variant="outline"
            aria-label="Refresh diagnostics"
            :disabled="isInspecting"
            @click="refreshDiagnostics"
            >{{ isInspecting ? "Checking…" : "Refresh diagnostics" }}</Button
          >
        </div>
        <p
          v-if="isInspecting"
          role="status"
          class="mt-2 text-sm text-(--muted-text)"
        >
          Checking dependencies…
        </p>
        <template v-if="diagnostics">
          <p class="mt-3 text-sm">
            {{
              isPlaybackReady
                ? "Playback tools are ready."
                : "Install missing tools to enable playback and import."
            }}
          </p>
          <p class="mt-1 text-xs text-(--muted-text)">
            GMusic {{ diagnostics.appVersion }} · {{ diagnostics.platform }}
          </p>
          <ul class="mt-3 divide-y divide-(--line)">
            <li
              v-for="dependency in diagnostics.dependencies"
              :key="dependency.name"
              class="py-2 text-sm"
            >
              <p class="font-medium">
                {{ dependency.name }} ·
                {{ dependency.available ? "Available" : "Missing"
                }}<template v-if="dependency.version">
                  · {{ dependency.version }}</template
                >
              </p>
              <p class="mt-1 break-words text-xs text-(--muted-text)">
                {{ dependency.message }}
              </p>
              <p
                v-if="!dependency.available"
                class="mt-1 text-xs text-(--muted-text)"
              >
                Install {{ dependency.name }} with your system package manager,
                then refresh diagnostics.
              </p>
            </li>
          </ul>
          <h3 class="mt-4 text-sm font-semibold">Audio output</h3>
          <p class="mt-1 break-words text-sm text-(--muted-text)">
            {{ diagnostics.audioOutputPolicy }}
          </p>
          <p class="mt-2 text-sm text-(--muted-text)">
            Select the system default output in your operating system sound
            settings. If sound stops after a device change, pause playback,
            check the output device, then resume playback.
          </p>
          <Button
            type="button"
            variant="outline"
            aria-label="Copy support info"
            class="mt-3"
            @click="
              copyText(
                JSON.stringify(diagnostics, null, 2),
                'Support info copied.',
              )
            "
            >Copy support info</Button
          >
        </template>
        <p
          v-if="diagnosticsError"
          role="alert"
          class="window-alert-danger mt-3 break-words p-3 text-sm"
        >
          {{ diagnosticsError }}
        </p>
      </section>

      <section class="window-panel p-5" aria-labelledby="backup-heading">
        <h2 id="backup-heading" class="text-base font-semibold">
          Library backup
        </h2>
        <p class="mt-1 text-sm text-(--muted-text)">
          Save a consistent library backup. Account sessions and cached media
          are excluded.
        </p>
        <Button
          type="button"
          variant="outline"
          aria-label="Back up library"
          class="mt-3"
          :disabled="isBackingUp"
          @click="backUpLibrary"
          >{{ isBackingUp ? "Creating backup…" : "Back up library" }}</Button
        >
        <template v-if="backupPath">
          <p role="status" class="mt-3 text-sm">Library backup created.</p>
          <input
            aria-label="Backup path"
            readonly
            :value="backupPath"
            class="mt-2 w-full select-text rounded-lg border border-(--line-strong) bg-(--glass-control) p-2 text-xs"
          />
          <Button
            type="button"
            variant="outline"
            aria-label="Copy backup path"
            class="mt-2"
            @click="copyText(backupPath, 'Backup path copied.')"
            >Copy backup path</Button
          >
        </template>
        <p
          v-if="backupError"
          role="alert"
          class="window-alert-danger mt-3 break-words p-3 text-sm"
        >
          {{ backupError }}
        </p>
      </section>

      <p v-if="copyMessage" role="status" class="text-sm">{{ copyMessage }}</p>
      <p
        v-if="copyError"
        role="alert"
        class="window-alert-danger break-words p-3 text-sm"
      >
        {{ copyError }}
      </p>
      <div v-if="playbackError" class="window-alert-danger p-3 text-sm">
        <p role="alert" class="break-words">{{ playbackError }}</p>
        <Button
          type="button"
          variant="outline"
          class="mt-2"
          @click="emit('retry')"
          >Retry playback</Button
        >
      </div>

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
      <Button
        v-if="errorMessage"
        type="button"
        variant="outline"
        :disabled="isBusy"
        @click="updateStatus(youtubeAuthApi.inspect)"
        >Retry session check</Button
      >
    </div>
  </main>
</template>
