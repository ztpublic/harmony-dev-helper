package dev.harmony.plugin

import com.sun.net.httpserver.HttpExchange
import com.sun.net.httpserver.HttpServer
import java.io.ByteArrayOutputStream
import java.net.InetSocketAddress

object HarmonyWebviewServer {
  private const val HOST = "127.0.0.1"
  private const val PORT = 8790
  private var server: HttpServer? = null

  fun startIfNeeded(): String {
    if (server == null) {
      val created = HttpServer.create(InetSocketAddress(HOST, PORT), 0)
      created.createContext("/") { exchange ->
        handleRequest(exchange)
      }
      created.start()
      server = created
    }

    return "http://$HOST:$PORT/index.html"
  }

  private fun handleRequest(exchange: HttpExchange) {
    val path = if (exchange.requestURI.path == "/") {
      "/index.html"
    } else {
      exchange.requestURI.path
    }

    val resourcePath = "webview$path"
    val stream = HarmonyWebviewServer::class.java.classLoader.getResourceAsStream(resourcePath)

    if (stream == null) {
      val body = "Not found".toByteArray()
      exchange.responseHeaders.add("Content-Type", "text/plain; charset=utf-8")
      exchange.sendResponseHeaders(404, body.size.toLong())
      exchange.responseBody.use { it.write(body) }
      return
    }

    val bytes = stream.use {
      val output = ByteArrayOutputStream()
      it.copyTo(output)
      output.toByteArray()
    }

    exchange.responseHeaders.add("Content-Type", contentType(path))
    exchange.sendResponseHeaders(200, bytes.size.toLong())
    exchange.responseBody.use { it.write(bytes) }
  }

  private fun contentType(path: String): String {
    return when {
      path.endsWith(".html") -> "text/html; charset=utf-8"
      path.endsWith(".css") -> "text/css; charset=utf-8"
      path.endsWith(".js") -> "application/javascript; charset=utf-8"
      path.endsWith(".json") -> "application/json; charset=utf-8"
      path.endsWith(".svg") -> "image/svg+xml"
      path.endsWith(".png") -> "image/png"
      path.endsWith(".jpg") || path.endsWith(".jpeg") -> "image/jpeg"
      else -> "application/octet-stream"
    }
  }
}
