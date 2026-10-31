use crate::App;
use serde::{Deserialize, Serialize};
use sprite_core::{EditCommand, Point, Rgba, Snapshot};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolRequest {
    pub version: u32,
    pub expected_revision: u64,
    pub action: ToolAction,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ToolAction {
    InspectDocument {},
    GetPalette {},
    AddLayer {
        name: String,
    },
    AddFrame {},
    PaintRegion {
        layer_id: String,
        frame_id: String,
        points: Vec<Point>,
        color: Rgba,
    },
    RequestGeneration {
        model: String,
        prompt: String,
        mode: String,
    },
    PreviewCandidate {
        id: String,
    },
    ApplyCandidate {
        id: String,
    },
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderConfig {
    #[default]
    Disabled,
    Gpt {
        model: String,
    },
    Ollama {
        model: String,
    },
    LlamaCpp {
        model: String,
    },
}
impl ProviderConfig {
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }
}
/// Providers propose bounded requests. The trusted caller supplies approval separately.
pub trait AgentProvider {
    fn propose(&self, document: &Snapshot) -> Result<Vec<ToolRequest>, String>;
}
pub struct MockProvider {
    pub requests: Vec<ToolRequest>,
}
impl AgentProvider for MockProvider {
    fn propose(&self, _: &Snapshot) -> Result<Vec<ToolRequest>, String> {
        if self.requests.len() > 32 {
            Err("Too many tool requests".into())
        } else {
            Ok(self.requests.clone())
        }
    }
}
impl App {
    pub fn execute_tool(
        &self,
        request: ToolRequest,
        approved: bool,
    ) -> Result<serde_json::Value, String> {
        if request.version != 1 {
            return Err("Unsupported tool protocol version".into());
        }
        let mutation = !matches!(
            request.action,
            ToolAction::InspectDocument {}
                | ToolAction::GetPalette {}
                | ToolAction::PreviewCandidate { .. }
        );
        if mutation && !approved {
            return Err("User approval is required".into());
        }
        let revision = request.expected_revision;
        let value = match request.action {
            ToolAction::InspectDocument {} => serde_json::to_value(self.snapshot()?),
            ToolAction::GetPalette {} => serde_json::to_value(self.snapshot()?.palette),
            ToolAction::AddLayer { name } => {
                serde_json::to_value(self.command(revision, EditCommand::AddLayer { name })?)
            }
            ToolAction::AddFrame {} => {
                serde_json::to_value(self.command(revision, EditCommand::AddFrame {})?)
            }
            ToolAction::PaintRegion {
                layer_id,
                frame_id,
                points,
                color,
            } => serde_json::to_value(self.command(
                revision,
                EditCommand::Paint {
                    layer_id,
                    frame_id,
                    points,
                    color,
                },
            )?),
            ToolAction::RequestGeneration {
                model,
                prompt,
                mode,
            } => serde_json::to_value(self.submit(revision, &model, &prompt, &mode)?),
            ToolAction::PreviewCandidate { id } => {
                let jobs = super::locked(&self.generations)?;
                serde_json::to_value(&jobs.proposals.get(&id).ok_or("Unknown candidate")?.view)
            }
            ToolAction::ApplyCandidate { id } => serde_json::to_value(self.accept(&id, revision)?),
        };
        value.map_err(|_| "Could not encode tool response".into())
    }
}
