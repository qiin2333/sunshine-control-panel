// Native drag regions handle mouse input; touch/pen need explicit window movement.
export function overlayTouchDrag(readPosition, moveWindow, onError = console.warn) {
  let gesture = null
  let previous = null
  let writes = Promise.resolve()
  let disposed = false
  const pump = async (drag) => {
    if (drag.writing || !drag.origin || !drag.latest || drag.cancelled) return
    drag.writing = true
    try {
      while (drag.latest && !drag.cancelled && !disposed) {
        const point = drag.latest
        drag.latest = null
        const position = {
          x: Math.round(drag.origin.x + (point.x - drag.x) * drag.scale),
          y: Math.round(drag.origin.y + (point.y - drag.y) * drag.scale)
        }
        writes = writes.then(() => {
          if (!disposed && !drag.cancelled) return moveWindow(position)
        }).catch(onError)
        await writes
      }
    } finally { drag.writing = false }
  }
  const start = async (event) => {
    if (disposed || gesture || !event.isPrimary || !['touch', 'pen'].includes(event.pointerType) || event.button !== 0) return
    event.preventDefault()
    const drag = { id: event.pointerId, target: event.currentTarget, x: event.screenX, y: event.screenY }
    if (previous) previous.cancelled = true
    previous = drag
    gesture = drag
    try {
      drag.target.setPointerCapture(drag.id)
      await writes
      const { position, scale } = await readPosition()
      if (gesture !== drag || disposed) return
      drag.origin = position
      drag.scale = scale
      await pump(drag)
    } catch (error) { if (gesture === drag) cancel(); onError(error) }
  }
  const move = (event) => {
    if (event.pointerId !== gesture?.id) return
    event.preventDefault()
    gesture.latest = { x: event.screenX, y: event.screenY }
    void pump(gesture)
  }
  const release = (drag) => {
    gesture = null
    if (drag.target.hasPointerCapture(drag.id)) drag.target.releasePointerCapture(drag.id)
  }
  const end = (event) => {
    if (event.pointerId !== gesture?.id) return
    move(event)
    release(gesture)
  }
  function cancel(event) {
    if (!gesture || (event && event.pointerId !== gesture.id)) return
    gesture.cancelled = true
    release(gesture)
  }
  return { start, move, end, cancel, dispose() { disposed = true; cancel() }, get active() { return gesture !== null } }
}
