/** Only recognized historical layouts can be changed without overwriting custom mappings. */
export function canRepairGameInput(report) {
  return ['legacy_mapping', 'axes_fixed', 'navigation_fixed'].includes(report?.state)
}

/** Keep incomplete recovery visible as a failure even when its JSON report is valid. */
export function gameInputReportFailed(report) {
  return ['access_denied', 'inspection_failed', 'operation_failed', 'repair_failed', 'recovery_required'].includes(report?.state)
}

/** Limit restore choices to fixed identifiers accepted by the elevated helper. */
export function gameInputBackupIds(...reports) {
  return [...new Set(reports.flatMap(report => [report?.backup_id, ...(Array.isArray(report?.backups) ? report.backups : [])])
    .filter(id => typeof id === 'string' && /^[0-9a-f]{32}$/i.test(id)))]
}
