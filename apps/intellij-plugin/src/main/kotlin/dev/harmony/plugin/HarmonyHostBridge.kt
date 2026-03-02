package dev.harmony.plugin

import com.intellij.openapi.fileEditor.OpenFileDescriptor
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.LocalFileSystem
import org.json.JSONObject
import java.io.File

object HarmonyHostBridge {
  private const val CHANNEL = "harmony-host"

  fun handleInvoke(project: Project, rawRequest: String): String {
    val request = try {
      JSONObject(rawRequest)
    } catch (error: Exception) {
      return buildErrorResponse(
        id = "invalid",
        action = "ide.openFile",
        code = "INVALID_ARGS",
        message = "Invalid JSON payload: ${error.message ?: "unknown error"}"
      )
    }

    val id = request.optString("id").ifBlank { "invalid" }
    val payload = request.optJSONObject("payload")
      ?: return buildErrorResponse(id, "ide.openFile", "INVALID_ARGS", "`payload` must be an object")

    val action = payload.optString("action")
    if (request.optString("channel") != CHANNEL) {
      return buildErrorResponse(id, normalizeAction(action), "INVALID_ARGS", "Invalid host bridge channel")
    }

    if (request.optString("type") != "invoke") {
      return buildErrorResponse(id, normalizeAction(action), "INVALID_ARGS", "Invalid host bridge message type")
    }

    return when (action) {
      "ide.getCapabilities" -> {
        buildResultResponse(
          id = id,
          action = action,
          data = JSONObject().put(
            "capabilities",
            JSONObject()
              .put("ide.openFile", true)
              .put("ide.openPath", false)
              .put("ide.openExternal", false)
              .put("ide.openChat", false)
          )
        )
      }
      "ide.openFile" -> handleOpenFile(project, id, payload.opt("args"))
      "ide.openPath" -> buildNoopOpenResult(id, action)
      "ide.openExternal" -> buildNoopOpenResult(id, action)
      "ide.openChat" -> buildNoopOpenResult(id, action)
      else -> buildErrorResponse(id, "ide.openFile", "INVALID_ARGS", "Unsupported host bridge action: $action")
    }
  }

  private fun handleOpenFile(project: Project, id: String, argsRaw: Any?): String {
    val args = argsRaw as? JSONObject
      ?: return buildErrorResponse(id, "ide.openFile", "INVALID_ARGS", "`args` must be an object")

    val path = args.optString("path").trim()
    if (path.isEmpty()) {
      return buildErrorResponse(id, "ide.openFile", "INVALID_ARGS", "`path` must be a non-empty string")
    }

    val ioFile = File(path)
    if (!ioFile.isAbsolute) {
      return buildErrorResponse(id, "ide.openFile", "INVALID_ARGS", "`path` must be an absolute filesystem path")
    }

    val lineParsed = parsePositiveInt(args, "line")
    if (lineParsed.error != null) {
      return buildErrorResponse(id, "ide.openFile", "INVALID_ARGS", lineParsed.error)
    }

    val columnParsed = parsePositiveInt(args, "column")
    if (columnParsed.error != null) {
      return buildErrorResponse(id, "ide.openFile", "INVALID_ARGS", columnParsed.error)
    }

    val previewParsed = parseOptionalBoolean(args, "preview")
    if (previewParsed.error != null) {
      return buildErrorResponse(id, "ide.openFile", "INVALID_ARGS", previewParsed.error)
    }

    val preserveFocusParsed = parseOptionalBoolean(args, "preserveFocus")
    if (preserveFocusParsed.error != null) {
      return buildErrorResponse(id, "ide.openFile", "INVALID_ARGS", preserveFocusParsed.error)
    }

    // V1 intentionally accepts preview but does not currently apply it in IntelliJ.
    val preserveFocus = preserveFocusParsed.value ?: false

    val virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByIoFile(ioFile)
      ?: return buildErrorResponse(id, "ide.openFile", "FILE_NOT_FOUND", "File does not exist: $path")

    val line = (lineParsed.value ?: 1) - 1
    val column = (columnParsed.value ?: 1) - 1

    val opened = OpenFileDescriptor(project, virtualFile, line, column).navigate(!preserveFocus)
    if (!opened) {
      return buildErrorResponse(id, "ide.openFile", "OPEN_FAILED", "Failed to open file in editor")
    }

    return buildResultResponse(
      id = id,
      action = "ide.openFile",
      data = JSONObject().put("opened", true)
    )
  }

  private fun normalizeAction(action: String): String {
    return if (
      action == "ide.getCapabilities" ||
      action == "ide.openFile" ||
      action == "ide.openPath" ||
      action == "ide.openExternal" ||
      action == "ide.openChat"
    ) {
      action
    } else {
      "ide.openFile"
    }
  }

  private fun buildNoopOpenResult(id: String, action: String): String {
    return buildResultResponse(
      id = id,
      action = action,
      data = JSONObject().put("opened", false)
    )
  }

  private fun buildResultResponse(id: String, action: String, data: JSONObject): String {
    return JSONObject()
      .put("channel", CHANNEL)
      .put("id", id)
      .put("type", "result")
      .put(
        "payload",
        JSONObject()
          .put("action", normalizeAction(action))
          .put("data", data)
      )
      .toString()
  }

  private fun buildErrorResponse(
    id: String,
    action: String,
    code: String,
    message: String
  ): String {
    return JSONObject()
      .put("channel", CHANNEL)
      .put("id", id)
      .put("type", "error")
      .put(
        "payload",
        JSONObject()
          .put("action", normalizeAction(action))
          .put("code", code)
          .put("message", message)
      )
      .toString()
  }

  private data class ParseIntResult(
    val value: Int?,
    val error: String?
  )

  private data class ParseBooleanResult(
    val value: Boolean?,
    val error: String?
  )

  private fun parsePositiveInt(json: JSONObject, key: String): ParseIntResult {
    if (!json.has(key) || json.isNull(key)) {
      return ParseIntResult(value = null, error = null)
    }

    val raw = json.get(key)
    val value = when (raw) {
      is Int -> raw
      is Long -> {
        if (raw > Int.MAX_VALUE.toLong()) {
          return ParseIntResult(null, "`$key` must be a positive integer when provided")
        }
        raw.toInt()
      }
      is Double -> {
        if (raw % 1.0 != 0.0) {
          return ParseIntResult(null, "`$key` must be a positive integer when provided")
        }
        raw.toInt()
      }
      else -> return ParseIntResult(null, "`$key` must be a positive integer when provided")
    }

    if (value <= 0) {
      return ParseIntResult(null, "`$key` must be a positive integer when provided")
    }

    return ParseIntResult(value = value, error = null)
  }

  private fun parseOptionalBoolean(json: JSONObject, key: String): ParseBooleanResult {
    if (!json.has(key) || json.isNull(key)) {
      return ParseBooleanResult(value = null, error = null)
    }

    val raw = json.get(key)
    if (raw !is Boolean) {
      return ParseBooleanResult(value = null, error = "`$key` must be a boolean when provided")
    }

    return ParseBooleanResult(value = raw, error = null)
  }
}
