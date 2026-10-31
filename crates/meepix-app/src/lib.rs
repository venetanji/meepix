//! Native application boundary, independent of the Tauri window runtime.
mod tools;
pub use tools::*;

use easel_client::{Client, Receipt, Status};
use serde::{Deserialize, Serialize};
use sprite_core::{EditCommand, Editor, Snapshot};
use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Generation {
    pub id: String,
    pub status: String,
    pub model: String,
    pub prompt: String,
    pub document_id: String,
    pub base_revision: u64,
    pub layer_id: String,
    pub frame_id: String,
    pub width: u32,
    pub height: u32,
    pub pixels: Option<Vec<u8>>,
    pub error: Option<String>,
}
#[derive(Serialize)]
pub struct Models {
    pub configured: bool,
    pub models: Vec<String>,
}
struct Proposal {
    view: Generation,
    receipt: Receipt,
    epoch: u64,
}
struct Generations {
    client: Option<Client>,
    proposals: HashMap<String, Proposal>,
}
struct Document {
    editor: Editor,
    epoch: u64,
    saved_path: Option<PathBuf>,
}
pub struct App {
    document: Mutex<Document>,
    generations: Mutex<Generations>,
    storage: PathBuf,
}
fn locked<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, String> {
    mutex
        .lock()
        .map_err(|_| "Application state unavailable; restart MeePix".into())
}
fn io_error(_: std::io::Error) -> String {
    "Could not access project storage".into()
}

impl App {
    pub fn new(storage: PathBuf, client: Option<Client>) -> Result<Self, String> {
        fs::create_dir_all(storage.join("projects")).map_err(io_error)?;
        fs::create_dir_all(storage.join("candidates")).map_err(io_error)?;
        Ok(Self {
            document: Mutex::new(Document {
                editor: Editor::new(64, 64)?,
                epoch: 1,
                saved_path: None,
            }),
            generations: Mutex::new(Generations {
                client,
                proposals: HashMap::new(),
            }),
            storage,
        })
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(locked(&self.document)?.editor.snapshot())
    }
    pub fn frame(&self, id: &str) -> Result<Vec<u8>, String> {
        locked(&self.document)?.editor.frame_pixels(id)
    }
    pub fn command(&self, revision: u64, command: EditCommand) -> Result<Snapshot, String> {
        locked(&self.document)?.editor.execute(revision, command)
    }
    pub fn create(&self, width: u32, height: u32) -> Result<Snapshot, String> {
        let editor = Editor::new(width, height)?;
        let mut document = locked(&self.document)?;
        document.editor = editor;
        document.epoch += 1;
        document.saved_path = None;
        Ok(document.editor.snapshot())
    }
    fn project_path(&self, name: &str, extension: &str) -> Result<PathBuf, String> {
        if name.is_empty()
            || name.len() > 80
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(
                "Use a project name with 1–80 letters, numbers, hyphens or underscores".into(),
            );
        }
        let path = self
            .storage
            .join("projects")
            .join(format!("{name}.{extension}"));
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Project symlinks are not supported".into());
        }
        Ok(path)
    }
    pub fn save(&self, name: &str) -> Result<Snapshot, String> {
        let path = self.project_path(name, "spriteproj")?;
        let mut doc = locked(&self.document)?;
        if path.exists() && doc.saved_path.as_ref() != Some(&path) {
            return Err("That project name already exists. Choose a new name or load that project before saving.".into());
        }
        doc.editor.save(&path)?;
        doc.saved_path = Some(path);
        Ok(doc.editor.snapshot())
    }
    pub fn load(&self, name: &str) -> Result<Snapshot, String> {
        let path = self.project_path(name, "spriteproj")?;
        // A failed load never replaces the current document.
        let editor = Editor::load(&path)?;
        let mut doc = locked(&self.document)?;
        doc.editor = editor;
        doc.epoch += 1;
        doc.saved_path = Some(path);
        Ok(doc.editor.snapshot())
    }
    pub fn export(&self, name: &str) -> Result<String, String> {
        let path = self.project_path(name, "png")?;
        self.project_path(name, "json")?;
        locked(&self.document)?.editor.export_sheet(&path)?;
        Ok(path.to_string_lossy().into_owned())
    }
    pub fn models(&self) -> Result<Models, String> {
        let jobs = locked(&self.generations)?;
        match &jobs.client {
            None => Ok(Models {
                configured: false,
                models: vec![],
            }),
            Some(client) => Ok(Models {
                configured: true,
                models: client
                    .models()
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .filter(|id| id == "qwen-image-2.1")
                    .collect(),
            }),
        }
    }
    pub fn submit(
        &self,
        revision: u64,
        model: &str,
        prompt: &str,
        mode: &str,
    ) -> Result<Generation, String> {
        if model.is_empty() || model.len() > 256 || prompt.trim().is_empty() || prompt.len() > 32768
        {
            return Err("Invalid generation model or prompt (maximum 32768 bytes)".into());
        }
        if !matches!(mode, "generate" | "edit") {
            return Err("Unsupported generation mode".into());
        }
        let (snapshot, epoch, reference) = {
            let doc = locked(&self.document)?;
            let s = doc.editor.snapshot();
            if s.revision != revision {
                return Err("Stale document revision".into());
            }
            let reference = if mode == "edit" {
                Some(encode_png(
                    s.width,
                    s.height,
                    &doc.editor
                        .cel_pixels(&s.active_layer_id, &s.active_frame_id)?,
                )?)
            } else {
                None
            };
            (s, doc.epoch, reference)
        };
        let mut jobs = locked(&self.generations)?;
        if jobs.proposals.len() >= 32 {
            return Err("Too many candidates in this session; restart after saving".into());
        }
        let client = jobs.client.as_ref().ok_or(
            "Easel is not configured; set EASEL_URL and optionally EASEL_API_KEY before launch",
        )?;
        // Persist intent before the sole POST; failed submission is never retried automatically.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let intent = self
            .storage
            .join("candidates")
            .join(format!("submission-{stamp}.json"));
        write_json(
            &intent,
            &serde_json::json!({"phase":"submission_unknown","model":model,"prompt":prompt,"mode":mode,"document_id":snapshot.document_id,"base_revision":revision}),
        )?;
        // Qwen uses multiples of 32. Small assets are generated at 512 and reduced explicitly.
        let receipt = client
            .submit(model, prompt, 512, 512, reference)
            .map_err(|e| {
                format!("{e}. Do not automatically resubmit; check Easel before trying again.")
            })?;
        let view = Generation {
            id: receipt.id.clone(),
            status: if receipt.status == Status::Completed {
                "in_progress".into()
            } else {
                status(&receipt.status).into()
            },
            model: model.into(),
            prompt: prompt.into(),
            document_id: snapshot.document_id,
            base_revision: revision,
            layer_id: snapshot.active_layer_id,
            frame_id: snapshot.active_frame_id,
            width: snapshot.width,
            height: snapshot.height,
            pixels: None,
            error: None,
        };
        let proposal = Proposal {
            view,
            receipt,
            epoch,
        };
        // Keep the accepted receipt reachable in memory even if local persistence fails.
        let view = proposal.view.clone();
        jobs.proposals.insert(view.id.clone(), proposal);
        let persisted = (|| -> Result<(), String> {
            let directory = self.candidate_dir(&view.id)?;
            fs::create_dir_all(&directory).map_err(io_error)?;
            let proposal = jobs
                .proposals
                .get(&view.id)
                .ok_or("Missing accepted receipt")?;
            write_json(&directory.join("receipt.json"), &proposal.receipt)?;
            write_json(&directory.join("provenance.json"), &proposal.view)?;
            Ok(())
        })();
        if persisted.is_err() {
            let proposal = jobs
                .proposals
                .get_mut(&view.id)
                .ok_or("Missing accepted receipt")?;
            proposal.view.error = Some(format!("Easel accepted job {}, but local storage failed. Do not resubmit. Repair storage, then retry status for this candidate.", view.id));
            return Ok(proposal.view.clone());
        }
        let _ = fs::remove_file(intent);
        Ok(view)
    }
    fn candidate_dir(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err("Invalid candidate id".into());
        }
        Ok(self.storage.join("candidates").join(id))
    }
    pub fn poll(&self, id: &str) -> Result<Generation, String> {
        let mut jobs = locked(&self.generations)?;
        let p = jobs
            .proposals
            .get(id)
            .ok_or("Unknown candidate in this session")?;
        if !matches!(p.view.status.as_str(), "queued" | "in_progress" | "unknown") {
            return Ok(p.view.clone());
        }
        let directory = self.candidate_dir(id)?;
        fs::create_dir_all(&directory).map_err(io_error)?;
        write_json(&directory.join("receipt.json"), &p.receipt)?;
        write_json(&directory.join("provenance.json"), &p.view)?;
        let receipt = jobs
            .client
            .as_ref()
            .ok_or("Easel is not configured")?
            .poll(id)
            .map_err(|e| e.to_string())?;
        let directory = self.candidate_dir(id)?;
        fs::create_dir_all(&directory).map_err(io_error)?;
        write_json(&directory.join("receipt.json"), &receipt)?;
        let pixels = if receipt.status == Status::Completed {
            let originals = jobs
                .client
                .as_ref()
                .unwrap()
                .fetch_content(&receipt)
                .map_err(|e| e.to_string())?;
            // First slice requests one output; preserve every returned original regardless.
            for (i, bytes) in originals.iter().enumerate() {
                atomic_write(&directory.join(format!("original-{i}.png")), bytes)?;
            }
            let p = jobs.proposals.get(id).unwrap();
            Some(resize_png(
                originals
                    .first()
                    .ok_or("Completed job contained no image")?,
                p.view.width,
                p.view.height,
            )?)
        } else {
            None
        };
        let proposal = jobs.proposals.get_mut(id).unwrap();
        proposal.view.status = status(&receipt.status).into();
        proposal.receipt = receipt;
        proposal.view.pixels = pixels;
        proposal.view.error = match proposal.receipt.status {
            Status::Failed => Some("Easel generation failed; inspect backend logs".into()),
            Status::Cancelled => Some("Generation was interrupted by the backend".into()),
            Status::Unknown => Some("Unknown Easel job status".into()),
            _ => None,
        };
        if let Some(pixels) = &proposal.view.pixels {
            atomic_write(
                &directory.join("preview.png"),
                &encode_png(proposal.view.width, proposal.view.height, pixels)?,
            )?;
        }
        write_json(&directory.join("provenance.json"), &proposal.view)?;
        Ok(proposal.view.clone())
    }
    pub fn reject(&self, id: &str) -> Result<(), String> {
        let mut jobs = locked(&self.generations)?;
        let proposal = jobs.proposals.get_mut(id).ok_or("Unknown candidate")?;
        if proposal.view.status == "accepted" {
            return Err("Candidate was already accepted".into());
        }
        // This stops local monitoring only. It does not cancel a server job.
        proposal.view.status = "rejected".into();
        write_json(
            &self.candidate_dir(id)?.join("provenance.json"),
            &proposal.view,
        )
    }
    pub fn accept(&self, id: &str, revision: u64) -> Result<Snapshot, String> {
        let mut jobs = locked(&self.generations)?;
        let proposal = jobs.proposals.get_mut(id).ok_or("Unknown candidate")?;
        if proposal.view.status != "completed" {
            return Err("Only completed candidates can be accepted".into());
        }
        let mut doc = locked(&self.document)?;
        let current = doc.editor.snapshot();
        if current.document_id != proposal.view.document_id
            || current.revision != proposal.view.base_revision
            || revision != current.revision
            || doc.epoch != proposal.epoch
            || current.active_layer_id != proposal.view.layer_id
            || current.active_frame_id != proposal.view.frame_id
        {
            return Err("Candidate is stale; document changed since submission".into());
        }
        // Content/provenance already durable before document mutation.
        let result = doc.editor.execute(
            revision,
            EditCommand::ReplaceCel {
                layer_id: proposal.view.layer_id.clone(),
                frame_id: proposal.view.frame_id.clone(),
                pixels: proposal
                    .view
                    .pixels
                    .clone()
                    .ok_or("Candidate has no pixels")?,
            },
        )?;
        proposal.view.status = "accepted".into();
        // Acceptance metadata is best-effort; artwork already contains the persisted pixels.
        let _ = write_json(
            &self.candidate_dir(id)?.join("provenance.json"),
            &proposal.view,
        );
        Ok(result)
    }
}
fn status(status: &Status) -> &'static str {
    match status {
        Status::Queued => "queued",
        Status::InProgress => "in_progress",
        Status::Completed => "completed",
        Status::Failed => "failed",
        Status::Cancelled => "cancelled",
        Status::Unknown => "unknown",
    }
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    // Same-directory staging keeps an interrupted write away from the destination.
    let temp = path.with_extension("pending");
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    use std::io::Write;
    let result = (|| {
        let mut file = options.open(&temp).map_err(io_error)?;
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        fs::rename(&temp, path).map_err(io_error)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    atomic_write(
        path,
        &serde_json::to_vec_pretty(value).map_err(|_| "Could not encode receipt")?,
    )
}
fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut bytes, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|_| "Could not encode PNG")?;
        writer
            .write_image_data(pixels)
            .map_err(|_| "Could not encode PNG")?;
    }
    Ok(bytes)
}
fn resize_png(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: 64 * 1024 * 1024,
        },
    );
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|_| "Invalid candidate PNG")?;
    let (w, h) = (reader.info().width, reader.info().height);
    if w == 0 || h == 0 || w > 4096 || h > 4096 || reader.output_buffer_size() > 64 * 1024 * 1024 {
        return Err("Candidate dimensions exceed limits".into());
    }
    let mut decoded = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut decoded)
        .map_err(|_| "Invalid candidate PNG")?;
    let channels = info.color_type.samples();
    let mut out = vec![0; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let source = (((y as u64 * h as u64 / height as u64) * w as u64
                + x as u64 * w as u64 / width as u64)
                * channels as u64) as usize;
            let dest = ((y * width + x) * 4) as usize;
            let p = &decoded[source..source + channels];
            let rgba = match info.color_type {
                png::ColorType::Rgba => [p[0], p[1], p[2], p[3]],
                png::ColorType::Rgb => [p[0], p[1], p[2], 255],
                png::ColorType::GrayscaleAlpha => [p[0], p[0], p[0], p[1]],
                png::ColorType::Grayscale => [p[0], p[0], p[0], 255],
                _ => return Err("Unsupported candidate pixels".into()),
            };
            out[dest..dest + 4].copy_from_slice(&rgba);
        }
    }
    Ok(out)
}
