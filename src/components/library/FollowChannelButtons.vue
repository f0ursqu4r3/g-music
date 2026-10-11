<script setup lang="ts">
defineProps<{
  channels: { channelId: string; name: string }[]
  followedChannelIds?: string[]
  pendingIds?: string[]
}>()

const emit = defineEmits<{
  follow: [channel: { channelId: string; name: string }]
  unfollow: [channelId: string]
}>()
</script>

<template>
  <div class="mt-4 grid gap-2" data-follow-controls>
    <p v-if="channels.length === 0" class="m-0 text-xs text-(--muted-text)">
      Refresh track metadata to follow this artist. No YouTube channel is known yet.
    </p>
    <template v-else>
      <p v-if="channels.length > 1" class="m-0 text-xs text-(--muted-text)">
        This artist has tracks from several channels. Choose which to follow.
      </p>
      <button
        v-for="channel in channels"
        :key="channel.channelId"
        class="inline-flex items-center justify-center gap-1.5 rounded-md border border-(--line-strong) px-2.5 py-1.5 text-xs font-medium text-(--text) hover:bg-(--surface-muted) focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none disabled:cursor-default disabled:opacity-40"
        :aria-label="`${followedChannelIds?.includes(channel.channelId) ? 'Unfollow' : 'Follow'} ${channel.name}`"
        :disabled="pendingIds?.includes(channel.channelId)"
        type="button"
        @click="
          followedChannelIds?.includes(channel.channelId)
            ? emit('unfollow', channel.channelId)
            : emit('follow', channel)
        "
      >
        {{ followedChannelIds?.includes(channel.channelId) ? 'Unfollow' : 'Follow' }}
        {{ channel.name }}
      </button>
    </template>
  </div>
</template>
