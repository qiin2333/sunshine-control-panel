import test from 'node:test'
import assert from 'node:assert/strict'

import { chooseByDirection, readingOrderIndexes } from './useFocusNav.js'

/** 3 列网格，卡片 180x260，间距 20。 */
function grid(rows, cols) {
  const rects = []
  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      const left = 100 + col * 200
      const top = 100 + row * 280
      rects.push({ left, right: left + 180, top, bottom: top + 260 })
    }
  }
  return rects
}

test('moving down from a grid tile stays in the same column', () => {
  const rects = grid(3, 3)
  // 第一行中间那张
  const from = rects[1]
  const candidates = rects.map((rect, index) => (index === 1 ? null : rect))

  // 期待落到第二行中间（下标 4），而不是第二行左边或右边
  assert.equal(chooseByDirection('down', from, candidates), 4)
})

test('moving right from a grid tile picks the immediate neighbour', () => {
  const rects = grid(2, 3)
  const from = rects[0]
  const candidates = rects.map((rect, index) => (index === 0 ? null : rect))

  assert.equal(chooseByDirection('right', from, candidates), 1)
})

test('moving right from the last tile in a row finds nothing', () => {
  const rects = grid(2, 3)
  // 第一行最右边
  const from = rects[2]
  const candidates = rects.map((rect, index) => (index === 2 ? null : rect))

  // 纯方向判定不会换行；换行由调用方按阅读顺序兜底
  assert.equal(chooseByDirection('right', from, candidates), -1)
})

test('reading order wraps from the end of a row to the start of the next', () => {
  const rects = grid(2, 3)
  const order = readingOrderIndexes(rects)

  assert.deepEqual(order, [0, 1, 2, 3, 4, 5])
  // 第一行末尾 (2) 的下一个是第二行开头 (3)
  assert.equal(order[order.indexOf(2) + 1], 3)
})

test('a sidebar to the left wins over a far tile on the same row', () => {
  const sidebar = { left: 10, right: 80, top: 120, bottom: 180 }
  const nearTile = { left: 100, right: 280, top: 100, bottom: 360 }
  const from = { left: 300, right: 480, top: 100, bottom: 360 }

  const index = chooseByDirection('left', from, [sidebar, nearTile])
  assert.equal(index, 1, '左移应该先到相邻卡片，而不是一路跳到侧边栏')

  // 已经在最左边的卡片上时，左移才落到侧边栏
  const fromNear = nearTile
  assert.equal(chooseByDirection('left', fromNear, [sidebar, null]), 0)
})

test('rows offset by a couple of pixels still count as the same row', () => {
  // 亚像素差异不该被当成上下关系
  const from = { left: 100, right: 280, top: 100, bottom: 360 }
  const jittered = { left: 300, right: 480, top: 102, bottom: 362 }

  assert.equal(chooseByDirection('down', from, [jittered]), -1)
  assert.equal(chooseByDirection('right', from, [jittered]), 0)
})

test('unknown directions never match', () => {
  const rects = grid(2, 2)
  assert.equal(chooseByDirection('sideways', rects[0], rects.slice(1)), -1)
})
