use objdiff_core::{diff, obj};

mod common;

#[test]
#[cfg(feature = "arm")]
fn read_arm() {
    let diff_config = diff::DiffObjConfig { ..Default::default() };
    let obj = obj::read::parse(
        include_object!("data/arm/LinkStateItem.o"),
        &diff_config,
        diff::DiffSide::Base,
    )
    .unwrap();
    insta::assert_debug_snapshot!(obj);
    let symbol_idx =
        obj.symbols.iter().position(|s| s.name == "_ZN13LinkStateItem12OnStateLeaveEi").unwrap();
    let diff = diff::code::no_diff_code(&obj, symbol_idx, &diff_config).unwrap();
    insta::assert_debug_snapshot!(diff.instruction_rows);
    let output = common::display_diff(&obj, &diff, symbol_idx, &diff_config);
    insta::assert_snapshot!(output);
}

#[test]
#[cfg(feature = "arm")]
fn read_thumb() {
    let diff_config = diff::DiffObjConfig { ..Default::default() };
    let obj =
        obj::read::parse(include_object!("data/arm/thumb.o"), &diff_config, diff::DiffSide::Base)
            .unwrap();
    insta::assert_debug_snapshot!(obj);
    let symbol_idx = obj
        .symbols
        .iter()
        .position(|s| s.name == "THUMB_BRANCH_ServerDisplay_UncategorizedMove")
        .unwrap();
    let diff = diff::code::no_diff_code(&obj, symbol_idx, &diff_config).unwrap();
    insta::assert_debug_snapshot!(diff.instruction_rows);
    let output = common::display_diff(&obj, &diff, symbol_idx, &diff_config);
    insta::assert_snapshot!(output);
}

#[test]
#[cfg(feature = "arm")]
fn combine_text_sections() {
    let diff_config = diff::DiffObjConfig { combine_text_sections: true, ..Default::default() };
    let obj = obj::read::parse(
        include_object!("data/arm/enemy300.o"),
        &diff_config,
        diff::DiffSide::Base,
    )
    .unwrap();
    let symbol_idx = obj.symbols.iter().position(|s| s.name == "Enemy300Draw").unwrap();
    let diff = diff::code::no_diff_code(&obj, symbol_idx, &diff_config).unwrap();
    insta::assert_debug_snapshot!(diff.instruction_rows);
    let output = common::display_diff(&obj, &diff, symbol_idx, &diff_config);
    insta::assert_snapshot!(output);
}

#[test]
#[cfg(feature = "arm")]
fn thumb_short_data_mapping() {
    // When a .2byte directive is used in Thumb code, the assembler emits
    // $d/$t mapping symbols for a 2-byte data region. The disassembler must
    // not read 4 bytes as a .word when the next mapping symbol limits the
    // data region to 2 bytes.
    let diff_config = diff::DiffObjConfig::default();
    let obj = obj::read::parse(
        include_object!("data/arm/code_1_vblank.o"),
        &diff_config,
        diff::DiffSide::Base,
    )
    .unwrap();
    let symbol_idx = obj.symbols.iter().position(|s| s.name == "VBlankDMA_Level1").unwrap();
    let diff = diff::code::no_diff_code(&obj, symbol_idx, &diff_config).unwrap();
    let output = common::display_diff(&obj, &diff, symbol_idx, &diff_config);
    // .2byte data followed by Thumb code must not be merged into a 4-byte .word
    assert!(
        !output.contains(".word"),
        "2-byte data regions should not be decoded as 4-byte .word values.\n\
         The disassembler must respect mapping symbol boundaries."
    );
}

#[test]
#[cfg(feature = "arm")]
fn read_arm_v4bx() {
    // R_ARM_V4BX relocations mark `bx` instructions so the linker can optionally
    // rewrite them for ARMv4 interworking.
    let diff_config = diff::DiffObjConfig::default();
    let obj =
        obj::read::parse(include_object!("data/arm/v4bx.o"), &diff_config, diff::DiffSide::Base)
            .unwrap();
    let symbol_idx = obj.symbols.iter().position(|s| s.name == "v4bx_func").unwrap();
    let diff = diff::code::no_diff_code(&obj, symbol_idx, &diff_config).unwrap();
    let output = common::display_diff(&obj, &diff, symbol_idx, &diff_config);
    // The `bx lr` still disassembles normally; the V4BX hint is simply dropped.
    assert!(output.contains("bx"), "expected a `bx` instruction, got:\n{output}");
}

#[test]
#[cfg(feature = "arm")]
fn trim_trailing_hword() {
    let diff_config = diff::DiffObjConfig::default();
    let obj = obj::read::parse(
        include_object!("data/arm/issue_253.o"),
        &diff_config,
        diff::DiffSide::Base,
    )
    .unwrap();
    let symbol_idx = obj.symbols.iter().position(|s| s.name == "sub_8030F20").unwrap();
    let diff = diff::code::no_diff_code(&obj, symbol_idx, &diff_config).unwrap();
    insta::assert_debug_snapshot!(diff.instruction_rows);
    let output = common::display_diff(&obj, &diff, symbol_idx, &diff_config);
    insta::assert_snapshot!(output);
}

#[test]
#[cfg(feature = "arm")]
fn do_not_trim_trailing_relocations() {
    let diff_config = diff::DiffObjConfig::default();
    let obj = obj::read::parse(
        include_object!("data/arm/fake_tank.o"),
        &diff_config,
        diff::DiffSide::Base,
    )
    .unwrap();
    let symbol_idx = obj.symbols.iter().position(|s| s.name == "FakeTankIdleInit").unwrap();
    let diff = diff::code::no_diff_code(&obj, symbol_idx, &diff_config).unwrap();
    insta::assert_debug_snapshot!(diff.instruction_rows);
    let output = common::display_diff(&obj, &diff, symbol_idx, &diff_config);
    insta::assert_snapshot!(output);
}

/// Two objects with the same `caller` and `callee`, in that order, so `callee` comes right after
/// `caller`. `thumb_call_resolved.o`'s `bl callee` goes to a local label, so the assembler resolved
/// it with no relocation; `thumb_call_relocated.o`'s goes to a global symbol, with an
/// `R_ARM_THM_CALL`, and links to the same bytes.
#[cfg(feature = "arm")]
const THUMB_CALL_RESOLVED: &[u8] = include_object!("data/arm/thumb_call_resolved.o");
#[cfg(feature = "arm")]
const THUMB_CALL_RELOCATED: &[u8] = include_object!("data/arm/thumb_call_relocated.o");

/// The match percentage of `caller` when `target` is diffed against `base`.
#[cfg(feature = "arm")]
fn thumb_call_match_percent(
    target: &[u8],
    base: &[u8],
    function_reloc_diffs: diff::FunctionRelocDiffs,
) -> f32 {
    let diff_config = diff::DiffObjConfig { function_reloc_diffs, ..Default::default() };
    let target_obj = obj::read::parse(target, &diff_config, diff::DiffSide::Target).unwrap();
    let base_obj = obj::read::parse(base, &diff_config, diff::DiffSide::Base).unwrap();
    let diff = diff::diff_objs(
        Some(&target_obj),
        Some(&base_obj),
        None,
        &diff_config,
        &diff::MappingConfig::default(),
    )
    .unwrap();
    let symbol_idx = target_obj.symbols.iter().position(|s| s.name == "caller").unwrap();
    diff.left.as_ref().unwrap().symbols[symbol_idx].match_percent.unwrap()
}

#[test]
#[cfg(feature = "arm")]
fn relaxed_reloc_diffs_match_thumb_call_resolved_in_target() {
    let percent =
        |relocs| thumb_call_match_percent(THUMB_CALL_RESOLVED, THUMB_CALL_RELOCATED, relocs);
    // With relocation diffs relaxed, a call the target's assembler resolved matches the base's
    // relocated call.
    assert_eq!(percent(diff::FunctionRelocDiffs::None), 100.0);
    // Otherwise the missing relocation is still a difference.
    assert!(percent(diff::FunctionRelocDiffs::NameAddress) < 100.0);
}

#[test]
#[cfg(feature = "arm")]
fn relaxed_reloc_diffs_match_thumb_call_resolved_in_base() {
    let percent =
        |relocs| thumb_call_match_percent(THUMB_CALL_RELOCATED, THUMB_CALL_RESOLVED, relocs);
    // The same the other way round: the target's relocated call matches a call the base's
    // assembler resolved.
    assert_eq!(percent(diff::FunctionRelocDiffs::None), 100.0);
    assert!(percent(diff::FunctionRelocDiffs::NameAddress) < 100.0);
}
