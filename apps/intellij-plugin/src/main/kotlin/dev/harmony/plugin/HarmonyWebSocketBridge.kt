package dev.harmony.plugin

import java.io.File
import java.io.IOException
import java.net.InetSocketAddress
import java.net.Socket
import kotlin.concurrent.thread

object HarmonyWebSocketBridge {
  private const val HOST = "127.0.0.1"
  private const val PORT = 8789
  private const val READY_TIMEOUT_MS = 8_000L
  private const val READY_POLL_INTERVAL_MS = 150L

  @Volatile private var sidecarProcess: Process? = null

  fun wsUrl(): String = "ws://$HOST:$PORT"

  @Synchronized
  fun startIfNeeded() {
    if (isPortOpen()) {
      return
    }

    if (sidecarProcess?.isAlive != true) {
      sidecarProcess = spawnSidecarProcess()
    }

    if (!waitForPortReady(READY_TIMEOUT_MS)) {
      println("Harmony HDC bridge sidecar did not become ready on ${wsUrl()} within ${READY_TIMEOUT_MS}ms")
    }
  }

  @Synchronized
  fun stop() {
    val process = sidecarProcess ?: return

    if (process.isAlive) {
      process.destroy()
      process.waitFor(500, java.util.concurrent.TimeUnit.MILLISECONDS)
    }

    if (process.isAlive) {
      process.destroyForcibly()
    }

    sidecarProcess = null
  }

  private fun spawnSidecarProcess(): Process {
    val binaryOverride = System.getenv("HARMONY_HDC_BRIDGE_BIN")?.trim()?.takeIf { it.isNotEmpty() }
    val wsAddr = "$HOST:$PORT"

    val (command, workingDir) = if (binaryOverride != null) {
      Pair(listOf(binaryOverride, "--ws-addr", wsAddr), File(binaryOverride).parentFile)
    } else {
      val manifest = resolveManifestPath()
        ?: throw IllegalStateException(
          "Could not locate apps/hdc-bridge-rs/Cargo.toml. Set HARMONY_HDC_BRIDGE_BIN or HARMONY_HDC_BRIDGE_MANIFEST_PATH."
        )
      Pair(
        listOf("cargo", "run", "--manifest-path", manifest.absolutePath, "--", "--ws-addr", wsAddr),
        manifest.parentFile
      )
    }

    println("Harmony HDC bridge launching: ${command.joinToString(" ")}")

    val builder = ProcessBuilder(command)
    if (workingDir != null) {
      builder.directory(workingDir)
    }
    builder.redirectErrorStream(true)

    val process = builder.start()

    thread(name = "harmony-hdc-bridge-log", isDaemon = true) {
      process.inputStream.bufferedReader().useLines { lines ->
        lines.forEach { line ->
          println("[Harmony HDC Bridge] $line")
        }
      }
    }

    thread(name = "harmony-hdc-bridge-exit", isDaemon = true) {
      val code = process.waitFor()
      println("Harmony HDC bridge sidecar exited with code $code")
      if (sidecarProcess === process) {
        sidecarProcess = null
      }
    }

    return process
  }

  private fun resolveManifestPath(): File? {
    val manifestFromEnv = System.getenv("HARMONY_HDC_BRIDGE_MANIFEST_PATH")?.trim()
      ?.takeIf { it.isNotEmpty() }
      ?.let { File(it) }
    if (manifestFromEnv != null && manifestFromEnv.exists()) {
      return manifestFromEnv
    }

    var current = File(System.getProperty("user.dir")).absoluteFile
    while (true) {
      val candidate = File(current, "apps/hdc-bridge-rs/Cargo.toml")
      if (candidate.exists()) {
        return candidate
      }

      val parent = current.parentFile ?: break
      if (parent == current) {
        break
      }

      current = parent
    }

    return null
  }

  private fun waitForPortReady(timeoutMs: Long): Boolean {
    val deadline = System.currentTimeMillis() + timeoutMs

    while (System.currentTimeMillis() < deadline) {
      if (isPortOpen()) {
        return true
      }

      Thread.sleep(READY_POLL_INTERVAL_MS)
    }

    return isPortOpen()
  }

  private fun isPortOpen(): Boolean {
    return try {
      Socket().use { socket ->
        socket.connect(InetSocketAddress(HOST, PORT), 300)
      }
      true
    } catch (_: IOException) {
      false
    }
  }
}
