# MCP Extension API Reference

The Cursor Extension API provides programmatic access to register and manage MCP servers without modifying `mcp.json` files directly. This is particularly useful for enterprise environments, onboarding tools, or MDM systems that need to dynamically configure MCP servers.

## Overview

The MCP Extension API allows you to:

- Register MCP servers programmatically
- Support both HTTP/SSE and stdio transport methods
- Use the same configuration schema as `mcp.json`
- Manage server registration dynamically

This API is useful for organizations that need to:

- Deploy MCP configurations programmatically
- Integrate MCP setup into onboarding workflows
- Manage MCP servers through enterprise tools
- Avoid manual `mcp.json` modifications

## API Reference

### vscode.cursor.mcp.registerServer

Registers an MCP server that Cursor can communicate with.

**Signature:**

```
vscode.cursor.mcp.registerServer(config: ExtMCPServerConfig): void
```

**Parameters:**

- `config: ExtMCPServerConfig` - The server configuration object

### vscode.cursor.mcp.unregisterServer

Unregisters a previously registered MCP server.

**Signature:**

```
vscode.cursor.mcp.unregisterServer(serverName: string): void
```

**Parameters:**

- `serverName: string` - The name of the server to unregister

## Type Definitions

Use these TypeScript definitions for type checking:

```
declare module "vscode" {
  export namespace cursor {
    export namespace mcp {
      export interface StdioServerConfig {
        name: string;
        server: {
          command: string;
          args: string[];
          env: Record<string, string>;
        };
      }

      export interface RemoteServerConfig {
        name: string;
        server: {
          url: string;
          /**
           * Optional HTTP headers to include with every request to this server (e.g. for authentication).
           * The keys are header names and the values are header values.
           */
          headers?: Record<string, string>;
        };
      }

      export type ExtMCPServerConfig = StdioServerConfig | RemoteServerConfig;

      /**
       * Register an MCP server that the Cursor extension can communicate with.
       *
       * The server can be exposed either over HTTP(S) (SSE/streamable HTTP) **or** as a local
       * stdio process.
       */
      export const registerServer: (config: ExtMCPServerConfig) => void;
      export const unregisterServer: (serverName: string) => void;
    }
  }
}
```

## Configuration Types

### HTTP/SSE Server Configuration

For servers running on HTTP or Server-Sent Events:

```
interface RemoteServerConfig {
  name: string;
  server: {
    url: string;
    headers?: Record<string, string>;
  };
}
```

**Properties:**

- `name`: Unique identifier for the server
- `server.url`: The HTTP endpoint URL
- `server.headers` (optional): HTTP headers for authentication or other purposes

### Stdio Server Configuration

For local servers that communicate via standard input/output:

```
interface StdioServerConfig {
  name: string;
  server: {
    command: string;
    args: string[];
    env: Record<string, string>;
  };
}
```

**Properties:**

- `name`: Unique identifier for the server
- `server.command`: The executable command
- `server.args`: Command line arguments
- `server.env`: Environment variables

## Examples

### HTTP/SSE Server

Register a remote MCP server with authentication:

```
vscode.cursor.mcp.registerServer({
  name: "my-remote-server",
  server: {
    url: "https://api.example.com/mcp",
    headers: {
      Authorization: "Bearer your-token-here",
      "X-API-Key": "your-api-key",
    },
  },
});
```

### Stdio Server

Register a local MCP server:

```
vscode.cursor.mcp.registerServer({
  name: "my-local-server",
  server: {
    command: "python",
    args: ["-m", "my_mcp_server"],
    env: {
      API_KEY: "your-api-key",
      DEBUG: "true",
    },
  },
});
```

### Node.js Server

Register a Node.js-based MCP server:

```
vscode.cursor.mcp.registerServer({
  name: "nodejs-server",
  server: {
    command: "npx",
    args: ["-y", "@company/mcp-server"],
    env: {
      NODE_ENV: "production",
      CONFIG_PATH: "/path/to/config",
    },
  },
});
```

## Managing Servers

### Unregister a Server

```
// Unregister a previously registered server
vscode.cursor.mcp.unregisterServer("my-remote-server");
```

### Conditional Registration

```
// Only register if not already registered
if (!isServerRegistered("my-server")) {
  vscode.cursor.mcp.registerServer({
    name: "my-server",
    server: {
      url: "https://api.example.com/mcp",
    },
  });
}
```