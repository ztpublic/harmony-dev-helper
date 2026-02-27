package dev.harmony.plugin

import org.java_websocket.WebSocket
import org.java_websocket.handshake.ClientHandshake
import org.java_websocket.server.WebSocketServer
import java.net.InetSocketAddress

object HarmonyWebSocketBridge {
  private const val PORT = 8789
  private var server: WebSocketServer? = null

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
        val isPing = message.contains("\"type\":\"ping\"")
        val outgoing = if (isPing) {
          """{"id":"intellij-pong","type":"pong","payload":{"host":"intellij","note":"pong from intellij bridge"},"ts":${System.currentTimeMillis()}}"""
        } else {
          """{"id":"intellij-event","type":"event","payload":{"name":"invoke.received","data":{"host":"intellij"}},"ts":${System.currentTimeMillis()}}"""
        }

        conn.send(outgoing)
      }
    }

    server?.start()
  }

  fun port(): Int = PORT
}
