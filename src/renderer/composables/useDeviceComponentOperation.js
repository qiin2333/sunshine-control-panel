import { computed, shallowRef, watch } from 'vue'

const owner = shallowRef(null)
const lastChange = shallowRef(null)

export function useDeviceComponentOperation() {
  const token = Symbol('device-component-operation')
  let changed = false
  const busyElsewhere = computed(() => owner.value !== null && owner.value !== token)
  const markChanged = () => { changed = true }
  const onOtherChange = callback => watch(lastChange, event => {
    if (event?.owner !== token) callback()
  })
  const wrap = operation => async (...args) => {
    if (owner.value !== null) return
    owner.value = token
    changed = false
    try {
      return await operation(...args)
    } finally {
      owner.value = null
      if (changed) lastChange.value = { owner: token }
    }
  }
  return { busyElsewhere, markChanged, onOtherChange, wrap }
}
