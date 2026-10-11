import { getCurrentScope, onScopeDispose, ref } from 'vue'

import { followingApi, type FollowingSnapshot } from '@/api'

export type FollowingClient = typeof followingApi

function readErrorMessage(error: unknown): string {
  if (typeof error === 'string' && error.trim()) return error
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message: unknown }).message
    if (typeof message === 'string' && message.trim()) return message
  }
  return 'The following command failed.'
}

/**
 * Reads and changes followed artists. The backend owns all network polling.
 *
 * Every response is a whole snapshot, so a late one can undo newer state. Reads and checks apply
 * only if no follow or unfollow started or finished meanwhile. Otherwise the composable discards
 * them and re-inspects, which returns the authoritative backend state.
 */
export function useFollowing(client: FollowingClient = followingApi) {
  const snapshot = ref<FollowingSnapshot>({ artists: [] })
  const pendingChannelIds = ref<string[]>([])
  const isChecking = ref(false)
  const isLoading = ref(false)
  const isLoaded = ref(false)
  const errorMessage = ref('')

  let disposed = false
  let sequence = 0
  let appliedSequence = 0
  let mutationEpoch = 0
  let mutationsInFlight = 0
  let reconcileNeeded = false
  let refreshing: Promise<void> | null = null
  let rerunRefresh = false
  let retryAction: (() => Promise<void>) | null = null

  if (getCurrentScope()) onScopeDispose(() => (disposed = true))

  function apply(id: number, next: FollowingSnapshot): void {
    appliedSequence = Math.max(appliedSequence, id)
    snapshot.value = next
    isLoaded.value = true
    errorMessage.value = ''
    retryAction = null
  }

  function fail(id: number, error: unknown, action: () => Promise<void>): void {
    // A newer result already replaced this request, so its error is obsolete.
    if (id < appliedSequence) return
    errorMessage.value = readErrorMessage(error)
    retryAction = action
  }

  /** Reads and checks. Returns false when the response was stale and discarded. */
  async function read(
    operation: () => Promise<FollowingSnapshot>,
    action: () => Promise<void>,
  ): Promise<void> {
    const id = ++sequence
    const startEpoch = mutationEpoch
    try {
      const next = await operation()
      if (disposed || id < appliedSequence) return
      if (startEpoch !== mutationEpoch || mutationsInFlight > 0) return reconcile()
      apply(id, next)
    } catch (error) {
      if (disposed || startEpoch !== mutationEpoch || mutationsInFlight > 0) return
      fail(id, error, action)
    }
  }

  function reconcile(): void {
    if (mutationsInFlight > 0) reconcileNeeded = true
    else void refresh()
  }

  function refresh(): Promise<void> {
    if (disposed) return Promise.resolve()
    if (refreshing) {
      rerunRefresh = true
      return refreshing
    }
    isLoading.value = true
    refreshing = (async () => {
      try {
        do {
          rerunRefresh = false
          await read(() => client.inspect(), refresh)
        } while (rerunRefresh && !disposed)
      } finally {
        refreshing = null
        isLoading.value = false
      }
    })()
    return refreshing
  }

  async function change(
    channelId: string,
    operation: () => Promise<FollowingSnapshot>,
    action: () => Promise<void>,
  ): Promise<void> {
    if (pendingChannelIds.value.includes(channelId)) return
    pendingChannelIds.value = [...pendingChannelIds.value, channelId]
    mutationsInFlight++
    mutationEpoch++
    const id = ++sequence
    let result: FollowingSnapshot | undefined
    let failure: { error: unknown } | undefined
    try {
      result = await operation()
    } catch (error) {
      failure = { error }
    }
    mutationsInFlight--
    mutationEpoch++
    pendingChannelIds.value = pendingChannelIds.value.filter((pending) => pending !== channelId)
    if (disposed) return
    if (mutationsInFlight > 0) {
      reconcileNeeded = true
      return
    }
    if (reconcileNeeded) {
      // Competing writes finished in an unknown order. Ask the backend.
      reconcileNeeded = false
      await refresh()
      if (failure && id >= appliedSequence) fail(id, failure.error, action)
      return
    }
    if (failure) fail(id, failure.error, action)
    else if (result && id >= appliedSequence) apply(id, result)
  }

  function follow(channelId: string, name: string): Promise<void> {
    return change(
      channelId,
      () => client.follow(channelId, name),
      () => follow(channelId, name),
    )
  }

  function unfollow(channelId: string): Promise<void> {
    return change(
      channelId,
      () => client.unfollow(channelId),
      () => unfollow(channelId),
    )
  }

  async function check(channelId?: string): Promise<void> {
    if (isChecking.value || disposed) return
    isChecking.value = true
    try {
      await read(
        () => client.check(channelId),
        () => check(channelId),
      )
    } finally {
      isChecking.value = false
    }
  }

  async function retry(): Promise<void> {
    const action = retryAction ?? refresh
    errorMessage.value = ''
    retryAction = null
    await action()
  }

  return {
    snapshot,
    pendingChannelIds,
    isChecking,
    isLoading,
    isLoaded,
    errorMessage,
    refresh,
    follow,
    unfollow,
    check,
    retry,
    clearError(): void {
      errorMessage.value = ''
      retryAction = null
    },
  }
}
