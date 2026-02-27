package dev.harmony.plugin

import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.content.ContentFactory
import com.intellij.ui.jcef.JBCefApp
import com.intellij.ui.jcef.JBCefBrowser
import javax.swing.JLabel
import javax.swing.JPanel
import java.awt.BorderLayout
import java.net.URLEncoder
import java.nio.charset.StandardCharsets

class HarmonyToolWindowFactory : ToolWindowFactory {
  override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
    val panel = JPanel(BorderLayout())

    if (!JBCefApp.isSupported()) {
      panel.add(JLabel("JCEF is not supported in this IDE runtime."), BorderLayout.CENTER)
      val content = ContentFactory.getInstance().createContent(panel, "", false)
      toolWindow.contentManager.addContent(content)
      return
    }

    HarmonyWebSocketBridge.startIfNeeded()

    val baseUrl = System.getProperty("harmony.webview.url") ?: HarmonyWebviewServer.startIfNeeded()
    val wsUrl = "ws://127.0.0.1:${HarmonyWebSocketBridge.port()}"
    val encodedWsUrl = URLEncoder.encode(wsUrl, StandardCharsets.UTF_8)
    val fullUrl = "$baseUrl?host=intellij&wsUrl=$encodedWsUrl"

    val browser = JBCefBrowser(fullUrl)
    panel.add(browser.component, BorderLayout.CENTER)

    val content = ContentFactory.getInstance().createContent(panel, "", false)
    toolWindow.contentManager.addContent(content)
  }
}
