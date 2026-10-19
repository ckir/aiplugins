//! The rmcp server: twenty tools over the engine.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig};
use rmcp::ServerHandler;

use crate::clock::Clock;
use crate::engine::Engine;
use crate::tools::*;
use crate::transport::Transport;

pub struct OpencodeCtlServer<T: Transport, C: Clock> {
    engine: Arc<Engine<T, C>>,
    // `#[tool_handler(router = self.tool_router)]` reads it on every call.
    tool_router: ToolRouter<Self>,
}

impl<T: Transport, C: Clock> OpencodeCtlServer<T, C> {
    pub fn new(engine: Arc<Engine<T, C>>) -> Self {
        Self {
            engine,
            tool_router: Self::tool_router(),
        }
    }
}

/// The twenty tool names with their descriptions, in registration order.
pub fn tool_descriptions() -> Vec<(&'static str, &'static str)> {
    vec![
        ("service_status", "Report whether the opencode service is running and reachable (urls, pid, version) and the pairing of this server. Never starts the service. Spends worker model tokens: no."),
        ("service_start", "Start the opencode service if it is not running (idempotent) and return the same shape as service_status. Spends worker model tokens: no."),
        ("service_stop", "Stop the opencode service; needs confirm: true; refused on the OpenCode host. This stops the daemon for EVERY session on this machine; any later session tool restarts it; to stop one turn use session_interrupt. Spends worker model tokens: no."),
        ("service_restart", "Restart the opencode service and return service_status; needs confirm: true; refused on the OpenCode host. Pending permission requests are lost. Spends worker model tokens: no."),
        ("session_list", "List sessions (newest first) for the paired or current directory, or all with all: true. Returns {sessions: [{id, title, directory, outcome, updated, idle, tokens, cost}], next_cursor}. Spends worker model tokens: no."),
        ("session_create", "Create a session in a directory, optionally with title, agent, model (provider/model[#variant]), permissions and parent_id. Returns {session_id, directory, agent, model}. Spends worker model tokens: no."),
        ("session_get", "Read a session's state plus its pending permission requests and forms. Spends worker model tokens: no."),
        ("session_prompt", "Send a prompt to a session; then call session_wait with after=<message_id> on EVERY slice. Spends worker model tokens: yes."),
        ("session_wait", "Wait one slice for a session to settle after a prompt; call again with the same after while the state is running. Spends worker model tokens: no."),
        ("session_messages", "Read a session's messages, compacted. Spends worker model tokens: no."),
        ("session_changes", "Read the file changes a session made (diff over a message range). Spends worker model tokens: no."),
        ("workspace_status", "Read the version-control status of a workspace directory. Spends worker model tokens: no."),
        ("session_interrupt", "Interrupt the running turn of a session; {interrupted: false} means it was idle. Spends worker model tokens: no."),
        ("session_delete", "Delete a session; needs session_id and confirm: true and never defaults to the paired session. Spends worker model tokens: no."),
        ("permission_reply", "Answer a pending permission request with once, always or reject; always needs confirm: true. Spends worker model tokens: no."),
        ("form_reply", "Answer a pending form (the worker's question). Spends worker model tokens: no."),
        ("saved_permissions", "List saved permission rules, or remove one with remove_id and confirm: true. Spends worker model tokens: no."),
        ("opencode_operations", "List the operations of the opencode API (id, method, path, summary, mutating), optionally filtered. Spends worker model tokens: no."),
        ("opencode_api", "Call any opencode API operation by operationId (the escape hatch); mutations need allow_mutation and are gated. Spends worker model tokens: depends."),
        ("pair_status", "Read the pair record of this controller with the tui and claude liveness and the generation. Spends worker model tokens: no."),
    ]
}

#[rmcp::tool_router]
impl<T: Transport, C: Clock> OpencodeCtlServer<T, C> {
    #[rmcp::tool(
        description = "Report whether the opencode service is running and reachable (urls, pid, version) and the pairing of this server. Never starts the service. Spends worker model tokens: no."
    )]
    async fn service_status(
        &self,
        Parameters(args): Parameters<ServiceStatusArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.service_status(args).await))
    }

    #[rmcp::tool(
        description = "Start the opencode service if it is not running (idempotent) and return the same shape as service_status. Spends worker model tokens: no."
    )]
    async fn service_start(
        &self,
        Parameters(args): Parameters<ServiceStartArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.service_start(args).await))
    }

    #[rmcp::tool(
        description = "Stop the opencode service; needs confirm: true; refused on the OpenCode host. This stops the daemon for EVERY session on this machine; any later session tool restarts it; to stop one turn use session_interrupt. Spends worker model tokens: no."
    )]
    async fn service_stop(
        &self,
        Parameters(args): Parameters<ConfirmArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.service_stop(args).await))
    }

    #[rmcp::tool(
        description = "Restart the opencode service and return service_status; needs confirm: true; refused on the OpenCode host. Pending permission requests are lost. Spends worker model tokens: no."
    )]
    async fn service_restart(
        &self,
        Parameters(args): Parameters<ConfirmArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.service_restart(args).await))
    }

    #[rmcp::tool(
        description = "List sessions (newest first) for the paired or current directory, or all with all: true. Returns {sessions: [{id, title, directory, outcome, updated, idle, tokens, cost}], next_cursor}. Spends worker model tokens: no."
    )]
    async fn session_list(
        &self,
        Parameters(args): Parameters<SessionListArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.session_list(args).await))
    }

    #[rmcp::tool(
        description = "Create a session in a directory, optionally with title, agent, model (provider/model[#variant]), permissions and parent_id. Returns {session_id, directory, agent, model}. Spends worker model tokens: no."
    )]
    async fn session_create(
        &self,
        Parameters(args): Parameters<SessionCreateArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.session_create(args).await))
    }

    #[rmcp::tool(
        description = "Read a session's state plus its pending permission requests and forms. Spends worker model tokens: no."
    )]
    async fn session_get(
        &self,
        Parameters(args): Parameters<SessionGetArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.session_get(args).await))
    }

    #[rmcp::tool(
        description = "Send a prompt to a session; then call session_wait with after=<message_id> on EVERY slice. Spends worker model tokens: yes."
    )]
    async fn session_prompt(
        &self,
        Parameters(args): Parameters<SessionPromptArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.session_prompt(args).await))
    }

    #[rmcp::tool(
        description = "Wait one slice for a session to settle after a prompt; call again with the same after while the state is running. Spends worker model tokens: no."
    )]
    async fn session_wait(
        &self,
        Parameters(args): Parameters<SessionWaitArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.session_wait(args).await))
    }

    #[rmcp::tool(
        description = "Read a session's messages, compacted. Spends worker model tokens: no."
    )]
    async fn session_messages(
        &self,
        Parameters(args): Parameters<SessionMessagesArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.session_messages(args).await))
    }

    #[rmcp::tool(
        description = "Read the file changes a session made (diff over a message range). Spends worker model tokens: no."
    )]
    async fn session_changes(
        &self,
        Parameters(args): Parameters<SessionChangesArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.session_changes(args).await))
    }

    #[rmcp::tool(
        description = "Read the version-control status of a workspace directory. Spends worker model tokens: no."
    )]
    async fn workspace_status(
        &self,
        Parameters(args): Parameters<WorkspaceStatusArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.workspace_status(args).await))
    }

    #[rmcp::tool(
        description = "Interrupt the running turn of a session; {interrupted: false} means it was idle. Spends worker model tokens: no."
    )]
    async fn session_interrupt(
        &self,
        Parameters(args): Parameters<SessionInterruptArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self
            .engine
            .render(self.engine.session_interrupt(args).await))
    }

    #[rmcp::tool(
        description = "Delete a session; needs session_id and confirm: true and never defaults to the paired session. Spends worker model tokens: no."
    )]
    async fn session_delete(
        &self,
        Parameters(args): Parameters<SessionDeleteArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.session_delete(args).await))
    }

    #[rmcp::tool(
        description = "Answer a pending permission request with once, always or reject; always needs confirm: true. Spends worker model tokens: no."
    )]
    async fn permission_reply(
        &self,
        Parameters(args): Parameters<PermissionReplyArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.permission_reply(args).await))
    }

    #[rmcp::tool(
        description = "Answer a pending form (the worker's question). Spends worker model tokens: no."
    )]
    async fn form_reply(
        &self,
        Parameters(args): Parameters<FormReplyArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.form_reply(args).await))
    }

    #[rmcp::tool(
        description = "List saved permission rules, or remove one with remove_id and confirm: true. Spends worker model tokens: no."
    )]
    async fn saved_permissions(
        &self,
        Parameters(args): Parameters<SavedPermissionsArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self
            .engine
            .render(self.engine.saved_permissions(args).await))
    }

    #[rmcp::tool(
        description = "List the operations of the opencode API (id, method, path, summary, mutating), optionally filtered. Spends worker model tokens: no."
    )]
    async fn opencode_operations(
        &self,
        Parameters(args): Parameters<OpencodeOperationsArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self
            .engine
            .render(self.engine.opencode_operations(args).await))
    }

    #[rmcp::tool(
        description = "Call any opencode API operation by operationId (the escape hatch); mutations need allow_mutation and are gated. Spends worker model tokens: depends."
    )]
    async fn opencode_api(
        &self,
        Parameters(args): Parameters<OpencodeApiArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.opencode_api(args).await))
    }

    #[rmcp::tool(
        description = "Read the pair record of this controller with the tui and claude liveness and the generation. Spends worker model tokens: no."
    )]
    async fn pair_status(
        &self,
        Parameters(args): Parameters<PairStatusArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(self.engine.render(self.engine.pair_status(args).await))
    }
}

#[rmcp::tool_handler(router = self.tool_router)]
impl<T: Transport, C: Clock> ServerHandler for OpencodeCtlServer<T, C> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                self.engine.host.binary_name(),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Drives an opencode service as a worker: service_status first, then session_create \
                 or session_list, session_prompt, and session_wait on every slice until the state \
                 settles. Read session_changes to verify the work. Every result is JSON; failures \
                 are {error: {code, message, hint?}}.",
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::SystemClock;
    use crate::transport::NotFoundTransport;

    #[test]
    fn descriptions_table_matches_the_router() {
        let router = OpencodeCtlServer::<NotFoundTransport, SystemClock>::tool_router();
        let listed = router.list_all();
        let table = tool_descriptions();
        assert_eq!(listed.len(), table.len());
        for (name, desc) in table {
            let t = listed.iter().find(|t| t.name == name).expect(name);
            assert_eq!(t.description.as_deref(), Some(desc), "{name}");
        }
    }
}
