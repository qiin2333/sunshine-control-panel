const delay = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds))

export const isVddRepairVerified = (status) => Boolean(
  status?.running
  && status?.version_match
  && status?.state === 'ready'
)

export async function installVddWithRecovery({
  install,
  getStatus,
  onStatus = () => {},
  timeoutMs = 120_000,
  pollIntervalMs = 1_500,
  verificationError = 'VDD installation finished, but the driver could not be verified.',
}) {
  const deadline = Date.now() + timeoutMs
  let settled = false

  const installOutcome = Promise.resolve()
    .then(() => install())
    .then(
      (result) => ({ kind: 'install', result }),
      (error) => ({ kind: 'install_error', error })
    )

  const verificationOutcome = (async () => {
    let lastError = null

    while (!settled && Date.now() < deadline) {
      try {
        const result = await getStatus()
        if (result?.success) {
          onStatus(result.data)
          if (isVddRepairVerified(result.data)) {
            return { kind: 'verified', result }
          }
        } else if (result?.message) {
          lastError = new Error(result.message)
        }
      } catch (error) {
        lastError = error
      }

      const remaining = deadline - Date.now()
      if (remaining > 0 && !settled) {
        await delay(Math.min(pollIntervalMs, remaining))
      }
    }

    return { kind: 'verification_timeout', error: lastError }
  })()

  const outcome = await Promise.race([installOutcome, verificationOutcome])

  if (outcome.kind === 'install_error') {
    settled = true
    throw outcome.error
  }

  if (outcome.kind === 'verified') {
    settled = true
    return { ...outcome.result, recovered: true }
  }

  if (outcome.kind === 'verification_timeout') {
    settled = true
    throw new Error(verificationError, { cause: outcome.error })
  }

  if (!outcome.result?.success) {
    settled = true
    throw new Error(outcome.result?.message || verificationError)
  }

  const verification = await verificationOutcome
  settled = true
  if (verification.kind !== 'verified') {
    throw new Error(verification.error?.message || verificationError, {
      cause: verification.error,
    })
  }

  return { ...outcome.result, status: verification.result.data, recovered: false }
}
