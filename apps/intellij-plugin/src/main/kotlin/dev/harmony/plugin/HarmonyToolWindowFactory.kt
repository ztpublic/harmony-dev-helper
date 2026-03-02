package dev.harmony.plugin

import com.intellij.openapi.Disposable
import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.content.ContentFactory
import com.intellij.ui.jcef.JBCefApp
import com.intellij.ui.jcef.JBCefBrowser
import com.intellij.ui.jcef.JBCefJSQuery
import org.cef.browser.CefBrowser
import org.cef.browser.CefFrame
import org.cef.handler.CefLoadHandlerAdapter
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

    try {
      HarmonyWebSocketBridge.startIfNeeded()
    } catch (error: Exception) {
      println("Harmony HDC bridge startup failed: ${error.message}")
    }

    val baseUrl = System.getProperty("harmony.webview.url") ?: HarmonyWebviewServer.startIfNeeded()
    val wsUrl = HarmonyWebSocketBridge.wsUrl()
    val encodedWsUrl = URLEncoder.encode(wsUrl, StandardCharsets.UTF_8)
    val fullUrl = "$baseUrl?host=intellij&wsUrl=$encodedWsUrl"

    val browser = JBCefBrowser(fullUrl)
    val hostBridgeQuery = JBCefJSQuery.create(browser)
    hostBridgeQuery.addHandler { request ->
      val response = HarmonyHostBridge.handleInvoke(project, request)
      JBCefJSQuery.Response(response)
    }

    browser.jbCefClient.addLoadHandler(object : CefLoadHandlerAdapter() {
      override fun onLoadEnd(cefBrowser: CefBrowser?, frame: CefFrame?, httpStatusCode: Int) {
        if (cefBrowser == null || frame == null || !frame.isMain) {
          return
        }

        val script = """
          (() => {
            if (typeof window.__HARMONY_INTELLIJ_HOST_INVOKE__ === "function") {
              return;
            }

            window.__HARMONY_INTELLIJ_HOST_INVOKE__ = (request) => {
              const requestJson = JSON.stringify(request);
              return new Promise((resolve, reject) => {
                ${hostBridgeQuery.inject(
                  "requestJson",
                  "response => { try { resolve(JSON.parse(response)); } catch (error) { reject(error instanceof Error ? error.message : String(error)); } }",
                  "(code, message) => reject(new Error(`[${'$'}{code}] ${'$'}{message}`))"
                )}
              });
            };
          })();
        """.trimIndent()

        cefBrowser.executeJavaScript(script, cefBrowser.url, 0)
      }
    }, browser.cefBrowser)

    panel.add(browser.component, BorderLayout.CENTER)

    val content = ContentFactory.getInstance().createContent(panel, "", false)
    content.setDisposer(object : Disposable {
      override fun dispose() {
        hostBridgeQuery.dispose()
        browser.dispose()
        HarmonyWebSocketBridge.stop()
      }
    })

    toolWindow.contentManager.addContent(content)
  }
}
