import assert from 'node:assert/strict'
import test from 'node:test'

import { installSelectedDualSensePackage } from './dualsenseInstallFlow.js'

test('installs the component from the package selected by the user', async () => {
  const installedPaths = []
  const result = await installSelectedDualSensePackage({
    packagePath: 'C:\\Packages\\Sunshine.Ds5Sidecar.x64.zip',
    installPackage: async (packagePath) => installedPaths.push(packagePath),
  })

  assert.deepEqual(result, {
    started: true,
    packagePath: 'C:\\Packages\\Sunshine.Ds5Sidecar.x64.zip',
  })
  assert.deepEqual(installedPaths, ['C:\\Packages\\Sunshine.Ds5Sidecar.x64.zip'])
})

test('does not start installation when package selection is canceled', async () => {
  let installCalled = false
  const result = await installSelectedDualSensePackage({
    packagePath: null,
    installPackage: async () => { installCalled = true },
  })

  assert.deepEqual(result, { started: false, packagePath: null })
  assert.equal(installCalled, false)
})
