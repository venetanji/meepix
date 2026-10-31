use sprite_core::{EditCommand, Editor, Point};
use std::io::{Read, Write};
use zip::{ZipArchive, ZipWriter, write::FileOptions};

#[test]
fn undo_redo_and_saved_state_preserve_four_frame_alpha_cels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sprite.spriteproj");
    let mut editor = Editor::new(16, 16).unwrap();
    let mut s = editor.snapshot();
    let layer = s.active_layer_id.clone();
    let mut expected = Vec::new();
    for i in 0..4u8 {
        if i > 0 { s = editor.execute(s.revision, EditCommand::AddFrame {}).unwrap(); }
        s = editor.execute(s.revision, EditCommand::Paint { layer_id: layer.clone(), frame_id: s.active_frame_id.clone(), points: vec![Point { x:15, y:15 }], color:[i, 90, 123, i * 64] }).unwrap();
        expected.push((s.active_frame_id.clone(), editor.cel_pixels(&layer, &s.active_frame_id).unwrap()));
    }
    editor.save(&path).unwrap();
    assert!(!editor.snapshot().dirty);
    let prior = s.revision;
    s = editor.execute(s.revision, EditCommand::Undo {}).unwrap();
    assert!(s.dirty && s.revision > prior);
    s = editor.execute(s.revision, EditCommand::Redo {}).unwrap();
    assert!(!s.dirty && s.revision > prior);
    assert!(editor.execute(prior, EditCommand::Undo {}).is_err());
    let loaded = Editor::load(&path).unwrap();
    for (frame, pixels) in expected { assert_eq!(loaded.cel_pixels(&layer, &frame).unwrap(), pixels); }
}

#[test]
fn corrupted_png_inside_valid_project_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let valid = dir.path().join("valid.spriteproj");
    let corrupt = dir.path().join("corrupt.spriteproj");
    Editor::new(16,16).unwrap().save(&valid).unwrap();
    let mut original = ZipArchive::new(std::fs::File::open(valid).unwrap()).unwrap();
    let mut out = ZipWriter::new(std::fs::File::create(&corrupt).unwrap());
    for i in 0..original.len() {
        let mut entry = original.by_index(i).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new(); entry.read_to_end(&mut bytes).unwrap();
        if name.ends_with(".png") { bytes.truncate(bytes.len()/2); }
        out.start_file(name, FileOptions::default()).unwrap(); out.write_all(&bytes).unwrap();
    }
    out.finish().unwrap();
    assert!(Editor::load(&corrupt).is_err());
}
