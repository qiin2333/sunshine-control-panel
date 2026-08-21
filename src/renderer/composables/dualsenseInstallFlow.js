export async function installSelectedDualSensePackage({ packagePath, installPackage }) {
  if (typeof packagePath !== 'string' || !packagePath) {
    return { started: false, packagePath: null }
  }

  await installPackage(packagePath)
  return { started: true, packagePath }
}
