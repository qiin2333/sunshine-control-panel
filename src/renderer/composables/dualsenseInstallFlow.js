export async function installSelectedDualSensePackages({ packagePaths, installPackages }) {
  const normalizedPaths = [...new Set(
    (Array.isArray(packagePaths) ? packagePaths : [packagePaths])
      .filter((path) => typeof path === 'string' && path),
  )]
  if (!normalizedPaths.length) return { started: false, packagePaths: [] }

  await installPackages(normalizedPaths)
  return { started: true, packagePaths: normalizedPaths }
}
