package dev.harmony.plugin

import org.java_websocket.WebSocket
import org.java_websocket.handshake.ClientHandshake
import org.java_websocket.server.WebSocketServer
import java.net.InetSocketAddress

object HarmonyWebSocketBridge {
  private const val PORT = 8789
  private val idPattern = Regex("\"id\"\\s*:\\s*\"([^\"]+)\"")
  private val typePattern = Regex("\"type\"\\s*:\\s*\"([^\"]+)\"")
  private val actionPattern = Regex("\"action\"\\s*:\\s*\"([^\"]+)\"")
  private var server: WebSocketServer? = null

  private fun extractMessageId(raw: String): String {
    return idPattern.find(raw)?.groupValues?.getOrNull(1)?.takeIf { it.isNotBlank() } ?: "intellij-error"
  }

  private fun extractMessageType(raw: String): String {
    return typePattern.find(raw)?.groupValues?.getOrNull(1)?.takeIf { it.isNotBlank() } ?: "unknown"
  }

  private fun extractAction(raw: String): String? {
    return actionPattern.find(raw)?.groupValues?.getOrNull(1)?.takeIf { it.isNotBlank() }
  }

  private fun escapeJson(value: String): String {
    return value.replace("\\", "\\\\").replace("\"", "\\\"")
  }

  private fun errorEnvelope(id: String, code: String, message: String): String {
    return """{"id":"${escapeJson(id)}","type":"error","payload":{"code":"$code","message":"${escapeJson(message)}"},"ts":${System.currentTimeMillis()}}"""
  }

  fun startIfNeeded() {
    if (server != null) {
      return
    }

    server = object : WebSocketServer(InetSocketAddress("127.0.0.1", PORT)) {
      override fun onStart() {
        println("Harmony IntelliJ websocket bridge listening on ws://127.0.0.1:$PORT")
      }

      override fun onOpen(conn: WebSocket, handshake: ClientHandshake) {
        // no-op
      }

      override fun onClose(conn: WebSocket, code: Int, reason: String, remote: Boolean) {
        // no-op
      }

      override fun onError(conn: WebSocket?, ex: Exception) {
        ex.printStackTrace()
      }

      override fun onMessage(conn: WebSocket, message: String) {
        val id = extractMessageId(message)
        val type = extractMessageType(message)
        val outgoing = if (type == "invoke") {
          val action = extractAction(message)
          val detail = if (action != null) {
            "Invoke action not implemented in IntelliJ host: $action"
          } else {
            "No invoke actions are implemented in the IntelliJ host yet."
          }

          errorEnvelope(id, "NOT_IMPLEMENTED", detail)
        } else {
          errorEnvelope(id, "UNSUPPORTED_MESSAGE_TYPE", "Unsupported message type: $type")
        }

        conn.send(outgoing)
      }
    }

    server?.start()
  }

  fun port(): Int = PORT
}
