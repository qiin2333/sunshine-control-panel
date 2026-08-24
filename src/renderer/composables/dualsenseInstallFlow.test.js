import assert from 'node:assert/strict'
import test from 'node:test'

import { installSelectedDualSensePackages } from './dualsenseInstallFlow.js'

test('installs the components from the packages selected by the user', async () => {
  const installCalls = []
  const packagePaths = [
    'C:\\Packages\\Sunshine.Ds5Sidecar.x64.zip',
    'C:\\Packages\\HIDMaestro-v1.6.2.zip',
    'C:\\Packages\\USBip-0.9.7.7-x64.exe',
  ]
  const result = await installSelectedDualSensePackages({
    packagePaths,
    installPackages: async (selectedPaths) => installCalls.push(selectedPaths),
  })

  assert.deepEqual(result, {
    started: true,
    packagePaths,
  })
  assert.deepEqual(installCalls, [packagePaths])
})

test('does not start installation when package selection is canceled', async () => {
  let installCalled = false
  const result = await installSelectedDualSensePackages({
    packagePaths: null,
    installPackages: async () => { installCalled = true },
  })

  assert.deepEqual(result, { started: false, packagePaths: [] })
  assert.equal(installCalled, false)
})

test('deduplicates selected package paths before installation', async () => {
  const packagePath = 'C:\\Packages\\HIDMaestro-v1.6.2.zip'
  let installedPaths

  await installSelectedDualSensePackages({
    packagePaths: [packagePath, packagePath],
    installPackages: async (selectedPaths) => { installedPaths = selectedPaths },
  })

  assert.deepEqual(installedPaths, [packagePath])
})
