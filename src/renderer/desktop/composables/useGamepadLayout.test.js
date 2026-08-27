import test from 'node:test'
import assert from 'node:assert/strict'

import { chipFor, detectLayout } from './useGamepadLayout.js'

/** 真实设备在 Windows/Chromium 上报告的 gamepad.id 样本。 */
const PS_IDS = [
  'DualSense Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 0ce6)',
  'Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 09cc)', // DualShock 4 蓝牙
  'DUALSENSE (STANDARD GAMEPAD Vendor: 054c Product: 0df2)',
  'PLAYSTATION(R)3 Controller (STANDARD GAMEPAD Vendor: 054c Product: 0268)',
]

const XBOX_IDS = [
  'Xbox 360 Controller (XInput STANDARD GAMEPAD Vendor: 045e Product: 028e)',
  'Xbox One Controller (STANDARD GAMEPAD Vendor: 045e Product: 02ea)',
  'Xbox Elite Wireless Controller (STANDARD GAMEPAD Vendor: 045e Product: 0b00)',
  'Generic USB Joystick (STANDARD GAMEPAD Vendor: 0079 Product: 0006)', // 杂牌落到 Xbox 布局
]

test('Sony 设备（含蓝牙改名）识别为 ps 布局', () => {
  for (const id of PS_IDS) {
    assert.equal(detectLayout(id), 'ps', `应识别为 ps: ${id}`)
  }
})

test('Xbox 与未知设备落到 xbox 布局（标准映射索引即 Xbox 约定）', () => {
  for (const id of XBOX_IDS) {
    assert.equal(detectLayout(id), 'xbox', `应识别为 xbox: ${id}`)
  }
})

test('空 id 与无手柄状态落到 xbox 布局', () => {
  assert.equal(detectLayout(''), 'xbox')
  assert.equal(detectLayout(null), 'xbox')
  assert.equal(detectLayout(undefined), 'xbox')
})

test('大小写与厂商号位置不敏感；"Xbox ... Wireless Controller" 不得误判', () => {
  assert.equal(detectLayout('vendor: 054C product: 0ce6'), 'ps')
  assert.equal(detectLayout('dualsense adaptation layer'), 'ps')
  // 锚定的 ^wireless controller：蓝牙 Sony 开头命中，Xbox 品牌词组不命中
  assert.equal(detectLayout('Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 09cc)'), 'ps')
  assert.equal(detectLayout('Xbox Elite Wireless Controller (STANDARD GAMEPAD Vendor: 045e)'), 'xbox')
  // vendor 号锚定到 Vendor 字段：非 Sony 手柄的 Product 恰为 054c 不得误判
  assert.equal(detectLayout('Generic Pad (STANDARD GAMEPAD Vendor: 0079 Product: 054c)'), 'xbox')
  assert.equal(detectLayout('Xbox One Controller (STANDARD GAMEPAD Vendor: 045e Product: 054c)'), 'xbox')
})

test('两套布局的按键符号语义对应：confirm/back 互为镜像', () => {
  const xbox = chipFor('confirm', 'xbox')
  const ps = chipFor('confirm', 'ps')
  assert.equal(xbox.glyph, 'A')
  assert.equal(ps.glyph, '✕')
  assert.equal(chipFor('back', 'xbox').glyph, 'B')
  assert.equal(chipFor('back', 'ps').glyph, '○')
  assert.equal(chipFor('search', 'ps').glyph, '△')
  assert.equal(chipFor('favorite', 'ps').glyph, '□')
  assert.equal(chipFor('pages', 'ps').glyph, 'L1/R1')
  assert.equal(chipFor('scroll', 'ps').glyph, 'L2/R2')
})

test('未知动作 id 不抛异常并返回中性空芯片', () => {
  const chip = chipFor('nonexistent', 'ps')
  assert.equal(chip.tone, 'neutral')
})
