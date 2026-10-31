use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::{write::FileOptions, ZipArchive, ZipWriter};

pub type Rgba = [u8; 4];
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub visible: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Frame {
    pub id: String,
    pub duration_ms: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub document_id: String,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub frames: Vec<Frame>,
    pub active_layer_id: String,
    pub active_frame_id: String,
    pub palette: Vec<Rgba>,
    pub pixels: Vec<u8>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub dirty: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditCommand {
    Paint {
        layer_id: String,
        frame_id: String,
        points: Vec<Point>,
        color: Rgba,
    },
    Fill {
        layer_id: String,
        frame_id: String,
        x: u32,
        y: u32,
        color: Rgba,
    },
    ReplaceCel {
        layer_id: String,
        frame_id: String,
        pixels: Vec<u8>,
    },
    AddLayer {
        name: String,
    },
    SetLayerVisibility {
        layer_id: String,
        visible: bool,
    },
    AddFrame {},
    SetDuration {
        frame_id: String,
        duration_ms: u32,
    },
    SelectLayer {
        layer_id: String,
    },
    SelectFrame {
        frame_id: String,
    },
    Undo {},
    Redo {},
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Document {
    document_id: String,
    width: u32,
    height: u32,
    layers: Vec<Layer>,
    frames: Vec<Frame>,
    palette: Vec<Rgba>,
    cel_ids: Vec<Vec<String>>,
    cels: Vec<Vec<Vec<u8>>>,
}
pub struct Editor {
    doc: Document,
    revision: u64,
    active_layer_id: String,
    active_frame_id: String,
    undo: Vec<Document>,
    redo: Vec<Document>,
    saved: RefCell<Option<Document>>,
}
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn id() -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{t:x}-{:x}-{:x}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}
fn valid_dimension(v: u32) -> bool {
    matches!(v, 16 | 32 | 64 | 128)
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
impl Editor {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        if !valid_dimension(width) || !valid_dimension(height) {
            return Err("invalid dimensions".into());
        }
        let l = Layer {
            id: id(),
            name: "Layer 1".into(),
            visible: true,
        };
        let f = Frame {
            id: id(),
            duration_ms: 100,
        };
        let doc = Document {
            document_id: id(),
            width,
            height,
            layers: vec![l.clone()],
            frames: vec![f.clone()],
            palette: vec![[0, 0, 0, 255], [255, 255, 255, 255]],
            cel_ids: vec![vec![id()]],
            cels: vec![vec![vec![0; (width * height * 4) as usize]]],
        };
        Ok(Self {
            doc,
            revision: 0,
            active_layer_id: l.id,
            active_frame_id: f.id,
            undo: vec![],
            redo: vec![],
            saved: RefCell::new(None),
        })
    }
    fn indices(&self, l: &str, f: &str) -> Result<(usize, usize), String> {
        Ok((
            self.doc
                .layers
                .iter()
                .position(|x| x.id == l)
                .ok_or("unknown layer")?,
            self.doc
                .frames
                .iter()
                .position(|x| x.id == f)
                .ok_or("unknown frame")?,
        ))
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            document_id: self.doc.document_id.clone(),
            revision: self.revision,
            width: self.doc.width,
            height: self.doc.height,
            layers: self.doc.layers.clone(),
            frames: self.doc.frames.clone(),
            active_layer_id: self.active_layer_id.clone(),
            active_frame_id: self.active_frame_id.clone(),
            palette: self.doc.palette.clone(),
            pixels: self
                .frame_pixels(&self.active_frame_id)
                .expect("active frame exists"),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            dirty: self.saved.borrow().as_ref() != Some(&self.doc),
        }
    }
    pub fn cel_pixels(&self, l: &str, f: &str) -> Result<Vec<u8>, String> {
        let (l, f) = self.indices(l, f)?;
        Ok(self.doc.cels[l][f].clone())
    }
    pub fn frame_pixels(&self, f: &str) -> Result<Vec<u8>, String> {
        let fi = self
            .doc
            .frames
            .iter()
            .position(|x| x.id == f)
            .ok_or("unknown frame")?;
        let mut out = vec![0; (self.doc.width * self.doc.height * 4) as usize];
        for (li, layer) in self.doc.layers.iter().enumerate() {
            if !layer.visible {
                continue;
            }
            for (dst, src) in out
                .chunks_exact_mut(4)
                .zip(self.doc.cels[li][fi].chunks_exact(4))
            {
                let sa = src[3] as u32;
                let da = dst[3] as u32;
                let alpha = sa * 255 + da * (255 - sa);
                if alpha == 0 {
                    continue;
                }
                for i in 0..3 {
                    dst[i] =
                        ((src[i] as u32 * sa * 255 + dst[i] as u32 * da * (255 - sa) + alpha / 2)
                            / alpha) as u8;
                }
                dst[3] = ((alpha + 127) / 255) as u8;
            }
        }
        Ok(out)
    }
    fn trim_history(v: &mut Vec<Document>) {
        let mut bytes = 0usize;
        let mut keep = 0;
        for d in v.iter().rev() {
            let size = (d.width * d.height * 4) as usize * d.layers.len() * d.frames.len();
            if keep >= 64 || (keep > 0 && bytes + size > 64 * 1024 * 1024) {
                break;
            }
            bytes += size;
            keep += 1;
        }
        if v.len() > keep {
            v.drain(..v.len() - keep);
        }
    }
    fn reconcile_selection(&mut self) {
        if !self.doc.layers.iter().any(|x| x.id == self.active_layer_id) {
            self.active_layer_id = self.doc.layers[0].id.clone();
        }
        if !self.doc.frames.iter().any(|x| x.id == self.active_frame_id) {
            self.active_frame_id = self.doc.frames[0].id.clone();
        }
    }
    pub fn execute(
        &mut self,
        expected_revision: u64,
        command: EditCommand,
    ) -> Result<Snapshot, String> {
        if expected_revision != self.revision {
            return Err("stale revision".into());
        }
        match command {
            EditCommand::SelectLayer { layer_id } => {
                if !self.doc.layers.iter().any(|x| x.id == layer_id) {
                    return Err("unknown layer".into());
                }
                self.active_layer_id = layer_id;
                return Ok(self.snapshot());
            }
            EditCommand::SelectFrame { frame_id } => {
                if !self.doc.frames.iter().any(|x| x.id == frame_id) {
                    return Err("unknown frame".into());
                }
                self.active_frame_id = frame_id;
                return Ok(self.snapshot());
            }
            EditCommand::Undo {} => {
                let next_revision = self.revision.checked_add(1).ok_or("revision overflow")?;
                let prev = self.undo.pop().ok_or("nothing to undo")?;
                self.redo.push(std::mem::replace(&mut self.doc, prev));
                Self::trim_history(&mut self.redo);
                self.revision = next_revision;
                self.reconcile_selection();
                return Ok(self.snapshot());
            }
            EditCommand::Redo {} => {
                let next_revision = self.revision.checked_add(1).ok_or("revision overflow")?;
                let next = self.redo.pop().ok_or("nothing to redo")?;
                self.undo.push(std::mem::replace(&mut self.doc, next));
                Self::trim_history(&mut self.undo);
                self.revision = next_revision;
                self.reconcile_selection();
                return Ok(self.snapshot());
            }
            _ => {}
        }
        let next_revision = self.revision.checked_add(1).ok_or("revision overflow")?;
        let mut next = self.doc.clone();
        match command {
            EditCommand::Paint {
                layer_id,
                frame_id,
                points,
                color,
            } => {
                let (l, f) = self.indices(&layer_id, &frame_id)?;
                if points.is_empty()
                    || points.len() > 65536
                    || points
                        .iter()
                        .any(|p| p.x >= next.width || p.y >= next.height)
                {
                    return Err("invalid stroke".into());
                }
                let mut prev = &points[0];
                for p in &points {
                    let (mut x, mut y) = (prev.x as i32, prev.y as i32);
                    let (tx, ty) = (p.x as i32, p.y as i32);
                    let dx = (tx - x).abs();
                    let sx = if x < tx { 1 } else { -1 };
                    let dy = -(ty - y).abs();
                    let sy = if y < ty { 1 } else { -1 };
                    let mut err = dx + dy;
                    loop {
                        let i = ((y as u32 * next.width + x as u32) * 4) as usize;
                        next.cels[l][f][i..i + 4].copy_from_slice(&color);
                        if x == tx && y == ty {
                            break;
                        }
                        let e = 2 * err;
                        if e >= dy {
                            err += dy;
                            x += sx;
                        }
                        if e <= dx {
                            err += dx;
                            y += sy;
                        }
                    }
                    prev = p;
                }
            }
            EditCommand::Fill {
                layer_id,
                frame_id,
                x,
                y,
                color,
            } => {
                let (l, f) = self.indices(&layer_id, &frame_id)?;
                if x >= next.width || y >= next.height {
                    return Err("invalid coordinate".into());
                }
                let w = next.width as usize;
                let h = next.height as usize;
                let cel = &mut next.cels[l][f];
                let start = y as usize * w + x as usize;
                let old: [u8; 4] = cel[start * 4..start * 4 + 4].try_into().unwrap();
                if old != color {
                    let mut q = VecDeque::from([start]);
                    cel[start * 4..start * 4 + 4].copy_from_slice(&color);
                    while let Some(p) = q.pop_front() {
                        let x = p % w;
                        let y = p / w;
                        let mut visit = |n: usize| {
                            if cel[n * 4..n * 4 + 4] == old {
                                cel[n * 4..n * 4 + 4].copy_from_slice(&color);
                                q.push_back(n);
                            }
                        };
                        if x > 0 {
                            visit(p - 1)
                        }
                        if x + 1 < w {
                            visit(p + 1)
                        }
                        if y > 0 {
                            visit(p - w)
                        }
                        if y + 1 < h {
                            visit(p + w)
                        }
                    }
                }
            }
            EditCommand::ReplaceCel {
                layer_id,
                frame_id,
                pixels,
            } => {
                let (l, f) = self.indices(&layer_id, &frame_id)?;
                if pixels.len() != (next.width * next.height * 4) as usize {
                    return Err("invalid cel length".into());
                }
                next.cels[l][f] = pixels;
            }
            EditCommand::AddLayer { name } => {
                if next.layers.len() >= 16 || name.trim().is_empty() || name.len() > 256 {
                    return Err("invalid layer".into());
                }
                let layer = Layer {
                    id: id(),
                    name,
                    visible: true,
                };
                self.active_layer_id = layer.id.clone();
                next.layers.push(layer);
                next.cel_ids
                    .push((0..next.frames.len()).map(|_| id()).collect());
                next.cels.push(vec![
                    vec![0; (next.width * next.height * 4) as usize];
                    next.frames.len()
                ]);
            }
            EditCommand::SetLayerVisibility { layer_id, visible } => {
                let i = next
                    .layers
                    .iter()
                    .position(|x| x.id == layer_id)
                    .ok_or("unknown layer")?;
                next.layers[i].visible = visible;
            }
            EditCommand::AddFrame {} => {
                if next.frames.len() >= 64 {
                    return Err("too many frames".into());
                }
                let fi = next
                    .frames
                    .iter()
                    .position(|x| x.id == self.active_frame_id)
                    .ok_or("unknown frame")?;
                let frame = Frame {
                    id: id(),
                    duration_ms: next.frames[fi].duration_ms,
                };
                self.active_frame_id = frame.id.clone();
                next.frames.push(frame);
                for ids in &mut next.cel_ids {
                    ids.push(id());
                }
                for c in &mut next.cels {
                    c.push(c[fi].clone());
                }
            }
            EditCommand::SetDuration {
                frame_id,
                duration_ms,
            } => {
                if !(20..=10000).contains(&duration_ms) {
                    return Err("invalid duration".into());
                }
                let f = next
                    .frames
                    .iter_mut()
                    .find(|x| x.id == frame_id)
                    .ok_or("unknown frame")?;
                f.duration_ms = duration_ms;
            }
            _ => unreachable!(),
        }
        self.undo.push(std::mem::replace(&mut self.doc, next));
        Self::trim_history(&mut self.undo);
        self.redo.clear();
        self.revision = next_revision;
        Ok(self.snapshot())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    document_id: String,
    width: u32,
    height: u32,
    layers: Vec<Layer>,
    frames: Vec<Frame>,
    palette: Vec<Rgba>,
    cel_ids: Vec<Vec<String>>,
}
fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut bytes, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(pixels).map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}
fn decode_png(bytes: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: 2 * 1024 * 1024,
    });
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    if info.width != w
        || info.height != h
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
    {
        return Err("invalid PNG format or dimensions".into());
    }
    let size = reader.output_buffer_size();
    if size != (w * h * 4) as usize {
        return Err("invalid PNG size".into());
    }
    let mut out = vec![0; size];
    let output = reader.next_frame(&mut out).map_err(|e| e.to_string())?;
    if output.buffer_size() != size {
        return Err("invalid PNG output".into());
    }
    Ok(out)
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().ok_or("invalid path")?.to_string_lossy();
    let tmp: PathBuf = parent.join(format!(".{name}.{}.tmp", id()));
    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
impl Editor {
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let d = &self.doc;
        let manifest = Manifest {
            version: 1,
            document_id: d.document_id.clone(),
            width: d.width,
            height: d.height,
            layers: d.layers.clone(),
            frames: d.frames.clone(),
            palette: d.palette.clone(),
            cel_ids: d.cel_ids.clone(),
        };
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let opts = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(&serde_json::to_vec(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        for (l, row) in d.cels.iter().enumerate() {
            for (f, cel) in row.iter().enumerate() {
                zip.start_file(format!("cels/{}.png", d.cel_ids[l][f]), opts)
                    .map_err(|e| e.to_string())?;
                zip.write_all(&encode_png(d.width, d.height, cel)?)
                    .map_err(|e| e.to_string())?;
            }
        }
        let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
        atomic_write(path, &bytes)?;
        *self.saved.borrow_mut() = Some(self.doc.clone());
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
            return Err("project too large".into());
        }
        let mut zip = ZipArchive::new(file).map_err(|e| e.to_string())?;
        if zip.len() > 1025 {
            return Err("too many entries".into());
        }
        let mut names = HashSet::new();
        for i in 0..zip.len() {
            let entry = zip.by_index(i).map_err(|e| e.to_string())?;
            if !names.insert(entry.name().to_string()) || entry.size() > 1024 * 1024 {
                return Err("invalid archive entry".into());
            }
        }
        let manifest: Manifest = {
            let mut entry = zip.by_name("manifest.json").map_err(|e| e.to_string())?;
            let mut b = Vec::new();
            entry.read_to_end(&mut b).map_err(|e| e.to_string())?;
            serde_json::from_slice(&b).map_err(|e| e.to_string())?
        };
        if manifest.version != 1
            || !valid_dimension(manifest.width)
            || !valid_dimension(manifest.height)
            || !valid_id(&manifest.document_id)
            || manifest.layers.is_empty()
            || manifest.layers.len() > 16
            || manifest.frames.is_empty()
            || manifest.frames.len() > 64
            || manifest.palette.len() > 256
            || manifest.cel_ids.len() != manifest.layers.len()
        {
            return Err("invalid manifest".into());
        }
        let mut ids = HashSet::new();
        ids.insert(manifest.document_id.clone());
        for l in &manifest.layers {
            if !valid_id(&l.id)
                || !ids.insert(l.id.clone())
                || l.name.trim().is_empty()
                || l.name.len() > 256
            {
                return Err("invalid layer".into());
            }
        }
        for f in &manifest.frames {
            if !valid_id(&f.id)
                || !ids.insert(f.id.clone())
                || !(20..=10000).contains(&f.duration_ms)
            {
                return Err("invalid frame".into());
            }
        }
        let mut cels = Vec::new();
        for row in &manifest.cel_ids {
            if row.len() != manifest.frames.len() {
                return Err("invalid cels".into());
            }
            let mut cel_row = Vec::new();
            for cid in row {
                if !valid_id(cid) || !ids.insert(cid.clone()) {
                    return Err("invalid cel ID".into());
                }
                let name = format!("cels/{cid}.png");
                let mut e = zip.by_name(&name).map_err(|e| e.to_string())?;
                let mut b = Vec::new();
                e.read_to_end(&mut b).map_err(|e| e.to_string())?;
                cel_row.push(decode_png(&b, manifest.width, manifest.height)?);
            }
            cels.push(cel_row);
        }
        if zip.len() != 1 + manifest.layers.len() * manifest.frames.len() {
            return Err("unexpected archive entry".into());
        }
        let doc = Document {
            document_id: manifest.document_id,
            width: manifest.width,
            height: manifest.height,
            layers: manifest.layers,
            frames: manifest.frames,
            palette: manifest.palette,
            cel_ids: manifest.cel_ids,
            cels,
        };
        let active_layer_id = doc.layers[0].id.clone();
        let active_frame_id = doc.frames[0].id.clone();
        Ok(Self {
            saved: RefCell::new(Some(doc.clone())),
            doc,
            revision: 0,
            active_layer_id,
            active_frame_id,
            undo: vec![],
            redo: vec![],
        })
    }
    pub fn export_frame(&self, path: &Path) -> Result<(), String> {
        atomic_write(
            path,
            &encode_png(
                self.doc.width,
                self.doc.height,
                &self.frame_pixels(&self.active_frame_id)?,
            )?,
        )
    }
    pub fn export_sheet(&self, path: &Path) -> Result<(), String> {
        let w = self.doc.width as usize;
        let h = self.doc.height as usize;
        let sheet_w = w * self.doc.frames.len();
        let mut sheet = vec![0; sheet_w * h * 4];
        for (i, f) in self.doc.frames.iter().enumerate() {
            let pixels = self.frame_pixels(&f.id)?;
            for y in 0..h {
                let dst = (y * sheet_w + i * w) * 4;
                sheet[dst..dst + w * 4].copy_from_slice(&pixels[y * w * 4..(y + 1) * w * 4]);
            }
        }
        let png = encode_png(sheet_w as u32, h as u32, &sheet)?;
        let frames:Vec<_>=self.doc.frames.iter().enumerate().map(|(i,f)|serde_json::json!({"id":f.id,"x":i*w,"y":0,"width":w,"height":h,"duration_ms":f.duration_ms})).collect();
        let json = serde_json::to_vec_pretty(
            &serde_json::json!({"version":1,"width":sheet_w,"height":h,"frames":frames}),
        )
        .map_err(|e| e.to_string())?;
        let metadata = path.with_extension("json");
        if metadata == path {
            return Err("PNG and metadata paths collide".into());
        }
        let parent = metadata
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let pending = parent.join(format!(".sheet-{}.tmp", id()));
        let backup = parent.join(format!(".sheet-{}.bak", id()));
        atomic_write(&pending, &json)?;
        let had_previous = metadata.exists();
        if had_previous {
            if let Err(e) = fs::rename(&metadata, &backup) {
                let _ = fs::remove_file(&pending);
                return Err(e.to_string());
            }
        }
        if let Err(e) = fs::rename(&pending, &metadata) {
            if had_previous {
                let _ = fs::rename(&backup, &metadata);
            }
            let _ = fs::remove_file(&pending);
            return Err(e.to_string());
        }
        if let Err(e) = atomic_write(path, &png) {
            let _ = fs::remove_file(&metadata);
            if had_previous {
                let _ = fs::rename(&backup, &metadata);
            }
            return Err(e);
        }
        if had_previous {
            let _ = fs::remove_file(&backup);
        }
        Ok(())
    }
}
