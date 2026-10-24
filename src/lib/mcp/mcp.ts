// Frontend glue for the MCP backend commands: the server settings, the activity log and the client entries.
import {invoke} from '@tauri-apps/api/core';

/** The MCP server settings and state. */
export interface McpView {
    enabled: boolean;
    port: number;
    token: string;
    allowAiSettings: boolean;
    url: string;
    running: boolean;
    error: string | null;
    bridgeCommand: string;
}

/** One tool call in the activity log. */
export interface McpActivity {
    at: number;
    client: string | null;
    tool: string;
    ok: boolean;
    message: string | null;
    durationMs: number;
}

export type McpClient = 'claude-desktop' | 'claude-code' | 'codex';

/** Whether a client is installed and whether Attributor's entry in its configuration is current. */
export interface McpClientStatus {
    client: McpClient;
    detected: boolean;
    configured: boolean;
    upToDate: boolean;
    files: string[];
    error: string | null;
}

export interface McpConfigChange {
    enabled?: boolean;
    port?: number;
    allowAiSettings?: boolean;
}

export const mcp = {
    getConfig: (): Promise<McpView> => invoke('mcp_get_config'),
    setConfig: (change: McpConfigChange): Promise<McpView> => invoke('mcp_set_config', {...change}),
    regenerateToken: (): Promise<McpView> => invoke('mcp_regenerate_token'),
    activity: (): Promise<McpActivity[]> => invoke('mcp_activity'),
    clearActivity: (): Promise<void> => invoke('mcp_clear_activity'),
    clients: (): Promise<McpClientStatus[]> => invoke('mcp_clients'),
    install: (client: McpClient): Promise<McpClientStatus> => invoke('mcp_install_client', {client}),
    uninstall: (client: McpClient): Promise<McpClientStatus> => invoke('mcp_uninstall_client', {client}),
    snippet: (client: McpClient): Promise<string> => invoke('mcp_client_snippet', {client})
};
