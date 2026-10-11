<script setup lang="ts">
import { Airplay, Check, Speaker } from 'lucide-vue-next'
import { ref } from 'vue'

import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useOutput, type OutputClient, type OutputSubscriber } from '@/composables/useOutput'

const props = withDefaults(
  defineProps<{
    client?: OutputClient
    subscribe?: OutputSubscriber | null
    size?: 'icon-sm' | 'icon-xs'
    buttonClass?: string
  }>(),
  { size: 'icon-sm', buttonClass: '' },
)

const output =
  props.client === undefined
    ? useOutput()
    : useOutput(props.client, props.subscribe === undefined ? null : props.subscribe)
const trigger = ref<InstanceType<typeof Button>>()

function onOpenChange(open: boolean): void {
  if (open) void output.refresh()
}

function triggerElement(): HTMLElement | undefined {
  const value = trigger.value as unknown as { $el?: HTMLElement } | undefined
  return value?.$el
}

async function chooseAirPlay(): Promise<void> {
  const element = triggerElement()
  // Let the menu close first, so the popover is not dismissed with it.
  await new Promise((resolve) => setTimeout(resolve, 0))
  if (element) await output.openAirPlayPicker(element)
}
</script>

<template>
  <div data-output-menu class="contents">
    <DropdownMenu @update:open="onOpenChange">
      <DropdownMenuTrigger as-child>
        <Button
          ref="trigger"
          :aria-label="`Audio output: ${output.activeLabel.value}`"
          :title="`Audio output: ${output.activeLabel.value}`"
          :class="buttonClass"
          :size="size"
          variant="ghost"
        >
          <Airplay v-if="output.isAirPlay.value" aria-hidden="true" />
          <Speaker v-else aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="min-w-56">
        <DropdownMenuLabel>Audio output</DropdownMenuLabel>
        <DropdownMenuItem
          v-for="device in output.state.value.devices"
          :key="device.id"
          role="menuitemradio"
          :aria-checked="
            !output.isAirPlay.value && device.id === output.state.value.selectedDeviceId
          "
          :data-output-device="device.id"
          @select="output.selectDevice(device.id)"
        >
          <span class="flex-1 truncate">{{ device.name }}</span>
          <Check
            v-if="!output.isAirPlay.value && device.id === output.state.value.selectedDeviceId"
            aria-hidden="true"
          />
        </DropdownMenuItem>
        <template v-if="output.state.value.airplayAvailable">
          <DropdownMenuSeparator />
          <DropdownMenuItem
            data-output-airplay
            role="menuitemradio"
            :aria-checked="output.isAirPlay.value"
            @select="chooseAirPlay"
          >
            <Airplay aria-hidden="true" />
            <span class="flex-1 truncate">
              {{ output.isAirPlay.value ? `AirPlay: ${output.activeLabel.value}` : 'AirPlay…' }}
            </span>
            <Check v-if="output.isAirPlay.value" aria-hidden="true" />
          </DropdownMenuItem>
        </template>
        <p
          v-if="output.errorMessage.value"
          role="alert"
          class="px-2 py-1.5 text-xs text-(--muted-text)"
        >
          {{ output.errorMessage.value }}
        </p>
      </DropdownMenuContent>
    </DropdownMenu>
  </div>
</template>
