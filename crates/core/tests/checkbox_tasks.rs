//! Markdown checkboxes (`- [ ]`, `* [x]`) are tasks, and toggling one keeps the file's own style.
use notatnik_core::{BlockOp, BlockRef, TaskState, apply, parse_page};

fn states(src: &str) -> Vec<Option<(TaskState, bool)>> {
    parse_page(src)
        .blocks
        .iter()
        .map(|b| b.task.as_ref().map(|t| (t.state, t.checkbox)))
        .collect()
}

#[test]
fn checkboxes_are_todo_and_done_tasks() {
    let src = "- [ ] open\n* [x] done\n- [X] also done\n- [] not\n- [ ]glued\n- [x]\n- TODO word\n- plain\n";
    assert_eq!(
        states(src),
        [
            Some((TaskState::Todo, true)),
            Some((TaskState::Done, true)),
            Some((TaskState::Done, true)),
            None,
            None,
            Some((TaskState::Done, true)),
            Some((TaskState::Todo, false)),
            None,
        ]
    );
    // the text after the marker is what is shown
    let p = parse_page(src);
    let t = p.blocks[0].task.as_ref().unwrap();
    assert_eq!(t.marker_span.slice(src), "[ ]");
}

fn set(src: &str, state: Option<TaskState>, done_at: Option<&str>) -> String {
    let p = parse_page(src);
    apply(
        src,
        &p,
        &BlockOp::SetTaskState {
            block: BlockRef::at(&[0]),
            state,
            done_at: done_at.map(str::to_string),
            completed_limit: None,
        },
    )
    .unwrap()
    .new_source
}

#[test]
fn toggling_a_checkbox_keeps_it_a_checkbox() {
    assert_eq!(
        set("* [ ] call\n", Some(TaskState::Done), None),
        "* [x] call\n"
    );
    assert_eq!(
        set("* [x] call\n", Some(TaskState::Todo), None),
        "* [ ] call\n"
    );
    // with a clock, as the app does it: no `done::` line in a checkbox file
    assert_eq!(
        set(
            "* [ ] call\n",
            Some(TaskState::Done),
            Some("2026-10-18T10:00")
        ),
        "* [x] call\n"
    );
    assert_eq!(
        set(
            "* [x] call\n",
            Some(TaskState::Todo),
            Some("2026-10-18T10:00")
        ),
        "* [ ] call\n"
    );
    // a state a checkbox cannot show is written as a word, from then on a word task
    assert_eq!(
        set("* [ ] call\n", Some(TaskState::Doing), None),
        "* DOING call\n"
    );
    assert_eq!(
        set(
            "* [ ] call\n",
            Some(TaskState::Doing),
            Some("2026-10-18T10:00")
        ),
        "* DOING call\n"
    );
    // taking the marker away
    assert_eq!(set("* [ ] call\n", None, None), "* call\n");
    // a word task is untouched by all this
    assert_eq!(
        set("- TODO call\n", Some(TaskState::Done), None),
        "- DONE call\n"
    );
    assert_eq!(
        set(
            "- TODO call\n",
            Some(TaskState::Done),
            Some("2026-10-18T10:00")
        ),
        "- DONE call\n  done:: 2026-10-18T10:00\n"
    );
}

#[test]
fn a_repeating_checkbox_is_unticked_again_and_moves_on() {
    let out = set(
        "- [ ] water\n  scheduled:: 2026-10-14\n  repeat:: every 2w\n",
        Some(TaskState::Done),
        Some("2026-10-14T08:00"),
    );
    assert!(out.starts_with("- [ ] water\n"), "{out}");
    assert!(out.contains("scheduled:: 2026-10-28"), "{out}");
    assert!(!out.contains("done::"), "{out}");
}

#[test]
fn text_blocks_have_no_task() {
    // a checkbox in text between bullets is text
    let src = "- a\n\n[ ] not a task, just text\n";
    let p = parse_page(src);
    assert!(p.blocks[1].task.is_none());
}
