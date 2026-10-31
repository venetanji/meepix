use sprite_core::{EditCommand, Editor, Point};

#[test]
fn stroke_is_atomic_interpolated_and_revision_checked() {
    let mut e = Editor::new(16, 16).unwrap();
    let s = e.snapshot();
    let paint = EditCommand::Paint {
        layer_id: s.active_layer_id.clone(),
        frame_id: s.active_frame_id.clone(),
        points: vec![Point { x: 0, y: 0 }, Point { x: 3, y: 0 }],
        color: [1, 2, 3, 4],
    };
    let after = e.execute(0, paint.clone()).unwrap();
    assert_eq!(
        &after.pixels[..16],
        &[1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4]
    );
    assert!(e.execute(0, paint).is_err());
    let undo = e.execute(after.revision, EditCommand::Undo {}).unwrap();
    assert_eq!(&undo.pixels[..16], &[0; 16]);
    assert!(undo.revision > after.revision);
    let redo = e.execute(undo.revision, EditCommand::Redo {}).unwrap();
    assert_eq!(
        &redo.pixels[..16],
        &[1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4]
    );
}

#[test]
fn frame_copy_is_independent_and_invalid_requests_are_atomic() {
    let mut e = Editor::new(16, 16).unwrap();
    let s = e.snapshot();
    let s = e
        .execute(
            0,
            EditCommand::Paint {
                layer_id: s.active_layer_id.clone(),
                frame_id: s.active_frame_id.clone(),
                points: vec![Point { x: 15, y: 15 }],
                color: [9, 8, 7, 0],
            },
        )
        .unwrap();
    let s = e.execute(s.revision, EditCommand::AddFrame {}).unwrap();
    let original = s.frames[0].id.clone();
    let new = s.active_frame_id.clone();
    assert_ne!(original, new);
    assert_eq!(
        e.cel_pixels(&s.active_layer_id, &original).unwrap(),
        e.cel_pixels(&s.active_layer_id, &new).unwrap()
    );
    assert!(e
        .execute(
            s.revision,
            EditCommand::Paint {
                layer_id: s.active_layer_id.clone(),
                frame_id: new.clone(),
                points: vec![Point { x: 16, y: 0 }],
                color: [1; 4]
            }
        )
        .is_err());
    let s = e
        .execute(
            s.revision,
            EditCommand::ReplaceCel {
                layer_id: s.active_layer_id.clone(),
                frame_id: new,
                pixels: vec![0; 1024],
            },
        )
        .unwrap();
    assert_ne!(
        e.cel_pixels(&s.active_layer_id, &original).unwrap(),
        e.cel_pixels(&s.active_layer_id, &s.active_frame_id)
            .unwrap()
    );
}

#[test]
fn project_round_trip_preserves_exact_cels_and_exports() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("art.spriteproj");
    let mut e = Editor::new(16, 16).unwrap();
    let s = e.snapshot();
    let mut pixels = vec![0; 1024];
    pixels[..4].copy_from_slice(&[9, 8, 7, 0]);
    let s = e
        .execute(
            0,
            EditCommand::ReplaceCel {
                layer_id: s.active_layer_id.clone(),
                frame_id: s.active_frame_id.clone(),
                pixels: pixels.clone(),
            },
        )
        .unwrap();
    let s = e.execute(s.revision, EditCommand::AddFrame {}).unwrap();
    let s = e
        .execute(
            s.revision,
            EditCommand::SetDuration {
                frame_id: s.active_frame_id.clone(),
                duration_ms: 250,
            },
        )
        .unwrap();
    e.save(&project).unwrap();
    assert!(!e.snapshot().dirty);
    let loaded = Editor::load(&project).unwrap();
    let t = loaded.snapshot();
    assert_eq!(t.frames, s.frames);
    assert_eq!(
        loaded
            .cel_pixels(&t.active_layer_id, &t.frames[0].id)
            .unwrap(),
        pixels
    );
    loaded.export_sheet(&dir.path().join("sheet.png")).unwrap();
    assert!(dir.path().join("sheet.json").exists());
    loaded.export_frame(&dir.path().join("frame.png")).unwrap();
}

#[test]
fn corrupt_project_is_rejected_and_failed_save_preserves_original() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("bad.spriteproj");
    std::fs::write(&p, b"not a zip").unwrap();
    assert!(Editor::load(&p).is_err());
    let e = Editor::new(16, 16).unwrap();
    assert!(e
        .save(&dir.path().join("missing").join("out.spriteproj"))
        .is_err());
    assert_eq!(std::fs::read(p).unwrap(), b"not a zip");
}

#[test]
fn sheet_export_failure_preserves_existing_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("sheet.png");
    std::fs::create_dir(&target).unwrap();
    let meta = dir.path().join("sheet.json");
    std::fs::write(&meta, b"original").unwrap();
    assert!(Editor::new(16, 16).unwrap().export_sheet(&target).is_err());
    assert_eq!(std::fs::read(meta).unwrap(), b"original");
}

#[test]
fn selections_do_not_mutate_history_and_layers_composite_alpha() {
    let mut e = Editor::new(16, 16).unwrap();
    let s = e.snapshot();
    let s = e
        .execute(
            0,
            EditCommand::Paint {
                layer_id: s.active_layer_id.clone(),
                frame_id: s.active_frame_id.clone(),
                points: vec![Point { x: 0, y: 0 }],
                color: [255, 0, 0, 255],
            },
        )
        .unwrap();
    let bottom = s.active_layer_id.clone();
    let s = e
        .execute(s.revision, EditCommand::AddLayer { name: "top".into() })
        .unwrap();
    let s = e
        .execute(
            s.revision,
            EditCommand::Paint {
                layer_id: s.active_layer_id.clone(),
                frame_id: s.active_frame_id.clone(),
                points: vec![Point { x: 0, y: 0 }],
                color: [0, 0, 255, 128],
            },
        )
        .unwrap();
    assert_eq!(&s.pixels[..4], &[127, 0, 128, 255]);
    let selected = e
        .execute(s.revision, EditCommand::SelectLayer { layer_id: bottom })
        .unwrap();
    assert_eq!(selected.revision, s.revision);
    assert_eq!(selected.can_undo, s.can_undo);
}

#[test]
fn fill_is_contiguous_and_invalid_replace_does_not_change_state() {
    let mut e = Editor::new(16, 16).unwrap();
    let s = e.snapshot();
    let s = e
        .execute(
            0,
            EditCommand::Paint {
                layer_id: s.active_layer_id.clone(),
                frame_id: s.active_frame_id.clone(),
                points: vec![Point { x: 1, y: 0 }, Point { x: 1, y: 15 }],
                color: [1, 1, 1, 255],
            },
        )
        .unwrap();
    assert!(e
        .execute(
            s.revision,
            EditCommand::ReplaceCel {
                layer_id: s.active_layer_id.clone(),
                frame_id: s.active_frame_id.clone(),
                pixels: vec![0; 3]
            }
        )
        .is_err());
    let s = e
        .execute(
            s.revision,
            EditCommand::Fill {
                layer_id: s.active_layer_id.clone(),
                frame_id: s.active_frame_id.clone(),
                x: 0,
                y: 0,
                color: [2, 3, 4, 255],
            },
        )
        .unwrap();
    assert_eq!(&s.pixels[..12], &[2, 3, 4, 255, 1, 1, 1, 255, 0, 0, 0, 0]);
}
