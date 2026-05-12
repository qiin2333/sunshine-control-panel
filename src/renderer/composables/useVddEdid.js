import { ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { vdd } from '../tauri-adapter.js'

const VALID_EDID_SIZES = [128, 256]
const EDID_HEADER_BYTES = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]

const fillPlaceholders = (message, replacements = {}) => Object.entries(replacements).reduce(
  (result, [key, value]) => result.replace(`{${key}}`, String(value)),
  message || ''
)

const getErrorMessage = (error, fallback = '') => {
  if (error instanceof Error && error.message) {
    return error.message
  }

  if (typeof error === 'string' && error.trim()) {
    return error
  }

  return fallback
}

export function useVddEdid({ t, settings }) {
  const edidFileExists = ref(false)
  const edidFilePath = ref('')
  const edidInfo = ref(null)

  const getVddText = (key, replacements = {}) => fillPlaceholders(t.value.vddSettings[key], replacements)

  const resetEdidState = () => {
    edidFileExists.value = false
    edidFilePath.value = ''
    edidInfo.value = null
  }

  const validateEdidChecksum = (data) => {
    if (!data || data.length < 128 || data.length % 128 !== 0) {
      return false
    }

    for (let offset = 0; offset < data.length; offset += 128) {
      let checksum = 0
      for (let index = offset; index < offset + 128; index += 1) {
        checksum = (checksum + data[index]) % 256
      }

      if (checksum !== 0) {
        return false
      }
    }

    return true
  }

  const getEdidFormatLabel = (size) => {
    if (size === 128) {
      return t.value.vddSettings.edidFormatBasic
    }

    if (size === 256) {
      return t.value.vddSettings.edidFormatCea
    }

    return t.value.vddSettings.edidFormatUnknown
  }

  const checkEdidFile = async () => {
    try {
      const pathResult = await vdd.getEdidFilePath()
      if (pathResult?.success) {
        edidFilePath.value = pathResult.data
      } else {
        edidFilePath.value = ''
      }

      const readResult = await vdd.readEdidFile()
      if (!readResult?.success) {
        resetEdidState()
        return
      }

      edidFileExists.value = true
      edidInfo.value = {
        size: readResult.data.length,
        checksumValid: validateEdidChecksum(readResult.data),
      }
    } catch {
      resetEdidState()
    }
  }

  const handleEdidToggle = (value) => {
    if (value && !edidFileExists.value) {
      ElMessage.warning(t.value.vddSettings.edidUploadFirst)
    }
  }

  const handleEdidFileChange = async (file) => {
    if (!file || !file.raw) {
      ElMessage.warning(t.value.vddSettings.edidSelectValid)
      return
    }

    const fileSize = file.raw.size
    if (!VALID_EDID_SIZES.includes(fileSize)) {
      ElMessage.error(getVddText('edidSizeInvalid', { size: fileSize }))
      return
    }

    try {
      const arrayBuffer = await file.raw.arrayBuffer()
      const uint8Array = new Uint8Array(arrayBuffer)
      const headerValid = EDID_HEADER_BYTES.every((byte, index) => uint8Array[index] === byte)

      if (!headerValid) {
        ElMessage.error(t.value.vddSettings.edidHeaderInvalid)
        return
      }

      if (!validateEdidChecksum(uint8Array)) {
        ElMessage.error(t.value.vddSettings.edidChecksumError)
        return
      }

      const result = await vdd.uploadEdidFile(Array.from(uint8Array))
      if (!result?.success) {
        throw new Error(result?.message || t.value.vddSettings.uploadFailed)
      }

      await checkEdidFile()
      ElMessage.success(t.value.vddSettings.edidUploadSuccess)
    } catch (error) {
      console.error('Upload EDID file error:', error)
      ElMessage.error(getVddText('uploadError', {
        error: getErrorMessage(error, t.value.vddSettings.uploadFailed),
      }))
    }
  }

  const downloadEdid = async () => {
    try {
      const result = await vdd.readEdidFile()
      if (!result?.success) {
        throw new Error(result?.message || t.value.vddSettings.readFailed)
      }

      const data = new Uint8Array(result.data)
      const blob = new Blob([data], { type: 'application/octet-stream' })
      const url = URL.createObjectURL(blob)
      const anchor = document.createElement('a')
      anchor.href = url
      anchor.download = 'user_edid.bin'
      document.body.appendChild(anchor)
      anchor.click()
      document.body.removeChild(anchor)
      URL.revokeObjectURL(url)
      ElMessage.success(t.value.vddSettings.edidDownloadSuccess)
    } catch (error) {
      console.error('Download EDID file error:', error)
      ElMessage.error(getVddText('downloadError', {
        error: getErrorMessage(error, t.value.vddSettings.readFailed),
      }))
    }
  }

  const removeEdidFile = async () => {
    try {
      await ElMessageBox.confirm(
        t.value.vddSettings.deleteEdidConfirm,
        t.value.vddSettings.deleteEdidTitle,
        {
          confirmButtonText: t.value.systemTools.confirm,
          cancelButtonText: t.value.systemTools.cancel,
          type: 'warning',
        }
      )

      const result = await vdd.deleteEdidFile()
      if (!result?.success) {
        throw new Error(result?.message || t.value.vddSettings.unknownError)
      }

      settings.edid.CustomEdid = false
      await checkEdidFile()
      ElMessage.success(t.value.vddSettings.deleteEdidSuccess)
    } catch (error) {
      if (error !== 'cancel') {
        ElMessage.error(getVddText('deleteEdidFailed', {
          error: getErrorMessage(error, t.value.vddSettings.unknownError),
        }))
      }
    }
  }

  return {
    edidFileExists,
    edidFilePath,
    edidInfo,
    getEdidFormatLabel,
    checkEdidFile,
    handleEdidToggle,
    handleEdidFileChange,
    downloadEdid,
    removeEdidFile,
  }
}
