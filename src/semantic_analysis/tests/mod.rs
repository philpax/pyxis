use crate::{
    grammar::test_aliases::*,
    semantic_analysis::{semantic_state::SemanticState, types::test_aliases::*},
};

use anyhow::Context;
use pretty_assertions::assert_eq;

mod alignment;
mod inheritance;
mod util;
use util::*;

#[test]
fn can_resolve_basic_struct() {
    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            TD::new([
                TS::field(V::Public, "field_1", T::ident("i32")),
                TS::field(V::Private, "_", T::unknown(4)),
                TS::field(V::Public, "field_2", T::ident("u64")),
            ])
            .with_attributes([A::align(8)]),
        )]),
        [SID::defined_resolved(
            SV::Public,
            "test::TestType",
            SISR {
                size: 16,
                alignment: 8,
                inner: STD::new()
                    .with_regions([
                        SR::field(SV::Public, "field_1", ST::raw("i32")),
                        SR::field(SV::Private, "_field_4", unknown(4)),
                        SR::field(SV::Public, "field_2", ST::raw("u64")),
                    ])
                    .into(),
            },
        )],
    );
}

#[test]
fn can_resolve_pointer_to_another_struct() {
    assert_ast_produces_type_definitions(
        M::new().with_definitions([
            ID::new(
                V::Public,
                "TestType1",
                TD::new([TS::field(V::Public, "field_1", T::ident("u64"))]),
            ),
            ID::new(
                V::Public,
                "TestType2",
                TD::new([
                    TS::field(V::Public, "field_1", T::ident("i32")),
                    TS::field(V::Public, "field_2", T::ident("TestType1"))
                        .with_attributes([A::address(8)]),
                    TS::field(V::Public, "field_3", T::ident("TestType1").const_pointer()),
                    TS::field(V::Public, "field_4", T::ident("TestType1").mut_pointer()),
                ])
                .with_attributes([A::align(8)]),
            ),
        ]),
        [
            SID::defined_resolved(
                SV::Public,
                "test::TestType1",
                SISR {
                    size: 8,
                    alignment: 8,
                    inner: STD::new()
                        .with_regions([SR::field(SV::Public, "field_1", ST::raw("u64"))])
                        .into(),
                },
            ),
            SID::defined_resolved(
                SV::Public,
                "test::TestType2",
                SISR {
                    size: 16 + 2 * pointer_size(),
                    alignment: 8,
                    inner: STD::new()
                        .with_regions([
                            SR::field(SV::Public, "field_1", ST::raw("i32")),
                            SR::field(SV::Private, "_field_4", unknown(4)),
                            SR::field(SV::Public, "field_2", ST::raw("test::TestType1")),
                            SR::field(
                                SV::Public,
                                "field_3",
                                ST::raw("test::TestType1").const_pointer(),
                            ),
                            SR::field(
                                SV::Public,
                                "field_4",
                                ST::raw("test::TestType1").mut_pointer(),
                            ),
                        ])
                        .into(),
                },
            ),
        ],
    );
}

#[test]
fn can_resolve_complex_type() {
    assert_ast_produces_type_definitions(
        M::new()
            .with_definitions([
                ID::new(
                    V::Public,
                    "TestType",
                    TD::new([
                        TS::field(V::Public, "field_1", T::ident("i32")),
                        TS::field(V::Private, "_", T::unknown(4)),
                    ]),
                ),
                ID::new(
                    V::Public,
                    "Singleton",
                    TD::new([
                        TS::field(V::Public, "max_num_1", T::ident("u16"))
                            .with_attributes([A::address(0x78)]),
                        TS::field(V::Public, "max_num_2", T::ident("u16")),
                        TS::field(V::Public, "test_type", T::ident("TestType"))
                            .with_attributes([A::address(0xA00)]),
                        TS::field(V::Public, "settings", T::unknown(804)),
                    ])
                    .with_attributes([A::size(0x1750), A::singleton(0x1_200_000)]),
                ),
            ])
            .with_impls([FB::new(
                "Singleton",
                [F::new(
                    V::Public,
                    "test_function",
                    [
                        Ar::MutSelf,
                        Ar::named("arg1", T::ident("TestType").mut_pointer()),
                        Ar::named("arg2", T::ident("i32")),
                        Ar::named("arg3", T::ident("u32").const_pointer()),
                    ],
                )
                .with_attributes([A::address(0x800_000)])
                .with_return_type(T::ident("TestType").mut_pointer())],
            )]),
        [
            SID::defined_resolved(
                SV::Public,
                "test::TestType",
                SISR {
                    size: 8,
                    alignment: pointer_size(),
                    inner: STD::new()
                        .with_regions([
                            SR::field(SV::Public, "field_1", ST::raw("i32")),
                            SR::field(SV::Private, "_field_4", unknown(4)),
                        ])
                        .into(),
                },
            ),
            SID::defined_resolved(
                SV::Public,
                "test::Singleton",
                SISR {
                    size: 0x1750,
                    alignment: pointer_size(),
                    inner: STD::new()
                        .with_regions([
                            SR::field(SV::Private, "_field_0", unknown(0x78)),
                            SR::field(SV::Public, "max_num_1", ST::raw("u16")),
                            SR::field(SV::Public, "max_num_2", ST::raw("u16")),
                            SR::field(SV::Private, "_field_7c", unknown(0x984)),
                            SR::field(SV::Public, "test_type", ST::raw("test::TestType")),
                            SR::field(SV::Public, "settings", unknown(804)),
                            SR::field(SV::Private, "_field_d2c", unknown(0xA24)),
                        ])
                        .with_free_functions([SF::new(SV::Public, "test_function")
                            .with_address(0x800_000)
                            .with_arguments([
                                SAr::MutSelf,
                                SAr::field(
                                    "arg1".to_string(),
                                    ST::raw("test::TestType").mut_pointer(),
                                ),
                                SAr::field("arg2", ST::raw("i32")),
                                SAr::field("arg3", ST::raw("u32").const_pointer()),
                            ])
                            .with_return_type(ST::raw("test::TestType").mut_pointer())])
                        .with_singleton(0x1_200_000)
                        .into(),
                },
            ),
        ],
    );
}

#[test]
fn will_eventually_terminate_with_an_unknown_type() {
    assert_ast_produces_failure(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType2",
            TD::new([TS::field(V::Private, "field_2", T::ident("TestType1"))]),
        )]),
        r#"type resolution will not terminate, failed on types: ["test::TestType2"] (resolved types: [])"#,
    );
}

#[test]
fn can_use_type_from_another_module() {
    let module1 = M::new()
        .with_uses([IP::from("module2::TestType2")])
        .with_definitions([ID::new(
            V::Public,
            "TestType1",
            TD::new([TS::field(V::Private, "field", T::ident("TestType2"))]),
        )]);
    let module2 = M::new().with_definitions([ID::new(
        V::Public,
        "TestType2",
        TD::new([TS::field(V::Private, "field", T::ident("u32"))]),
    )]);

    let mut semantic_state = SemanticState::new(4);
    semantic_state
        .add_module(&module1, &IP::from("module1"))
        .unwrap();
    semantic_state
        .add_module(&module2, &IP::from("module2"))
        .unwrap();
    let semantic_state = semantic_state.build().unwrap();

    let path = IP::from("module1::TestType1");
    let resolved_type = semantic_state
        .type_registry()
        .get(&path)
        .cloned()
        .context("failed to get type")
        .unwrap();
    assert_eq!(
        resolved_type,
        SID::defined_resolved(
            SV::Public,
            path.clone(),
            SISR {
                size: 4,
                alignment: 4,
                inner: STD::new()
                    .with_regions([SR::field(
                        SV::Private,
                        "field",
                        ST::raw("module2::TestType2")
                    )])
                    .into(),
            },
        )
    );
}

#[test]
fn will_fail_on_an_extern_without_size() {
    assert_ast_produces_failure(
        M::new().with_extern_types([("TestType".into(), vec![])]),
        "failed to find `size` attribute for extern type `TestType` in module `test`",
    );
}

#[test]
fn can_resolve_embed_of_an_extern() {
    assert_ast_produces_type_definitions(
        M::new()
            .with_extern_types([("TestType1".into(), vec![A::size(16), A::align(4)])])
            .with_definitions([ID::new(
                V::Public,
                "TestType2",
                TD::new([
                    TS::field(V::Public, "field_1", T::ident("u64")),
                    TS::field(V::Public, "field_2", T::ident("TestType1")),
                    TS::field(V::Public, "field_3", T::ident("TestType1").const_pointer()),
                    TS::field(V::Public, "field_4", T::ident("TestType1").mut_pointer()),
                ])
                .with_attributes([A::align(8)]),
            )]),
        [
            SID::defined_resolved(
                SV::Public,
                "test::TestType2",
                SISR {
                    size: 24 + 2 * pointer_size(),
                    alignment: 8,
                    inner: STD::new()
                        .with_regions([
                            SR::field(SV::Public, "field_1", ST::raw("u64")),
                            SR::field(SV::Public, "field_2", ST::raw("test::TestType1")),
                            SR::field(
                                SV::Public,
                                "field_3",
                                ST::raw("test::TestType1").const_pointer(),
                            ),
                            SR::field(
                                SV::Public,
                                "field_4",
                                ST::raw("test::TestType1").mut_pointer(),
                            ),
                        ])
                        .into(),
                },
            ),
            SID {
                visibility: SV::Public,
                path: "test::TestType1".into(),
                state: SISR {
                    size: 16,
                    alignment: 4,
                    inner: STD::new().with_regions([]).into(),
                }
                .into(),
                category: SIC::Extern,
            },
        ],
    );
}

#[test]
fn can_generate_vftable() {
    let vftable_type = ST::raw("test::TestTypeVftable").const_pointer();
    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            TD::new([TS::vftable(
                [
                    F::new(
                        V::Public,
                        "test_function0",
                        [
                            Ar::MutSelf,
                            Ar::named("arg0", T::ident("u32")),
                            Ar::named("arg1", T::ident("f32")),
                        ],
                    )
                    .with_return_type("i32"),
                    F::new(
                        V::Public,
                        "test_function1",
                        [
                            Ar::MutSelf,
                            Ar::named("arg0", T::ident("u32")),
                            Ar::named("arg1", T::ident("f32")),
                        ],
                    ),
                ],
                [A::size(4)],
            )]),
        )]),
        [
            // TestType
            SID::defined_resolved(
                SV::Public,
                "test::TestType",
                SISR {
                    size: pointer_size(),
                    alignment: pointer_size(),
                    inner: STD::new()
                        .with_regions([SR::field(SV::Private, "vftable", vftable_type.clone())])
                        .with_vftable_functions(
                            vftable_type,
                            [
                                SF::new(SV::Public, "test_function0")
                                    .with_arguments([
                                        SAr::MutSelf,
                                        SAr::field("arg0", ST::raw("u32")),
                                        SAr::field("arg1", ST::raw("f32")),
                                    ])
                                    .with_return_type(ST::raw("i32")),
                                SF::new(SV::Public, "test_function1").with_arguments([
                                    SAr::MutSelf,
                                    SAr::field("arg0", ST::raw("u32")),
                                    SAr::field("arg1", ST::raw("f32")),
                                ]),
                                make_vfunc(2),
                                make_vfunc(3),
                            ],
                        )
                        .into(),
                },
            ),
            // TestTypeVftable
            SID::defined_resolved(
                SV::Public,
                "test::TestTypeVftable",
                SISR {
                    size: 4 * pointer_size(),
                    alignment: pointer_size(),
                    inner: STD::new()
                        .with_regions([
                            SR::field(
                                SV::Public,
                                "test_function0",
                                ST::function(
                                    SCC::Thiscall,
                                    [
                                        ("this", ST::raw("test::TestType").mut_pointer()),
                                        ("arg0", ST::raw("u32")),
                                        ("arg1", ST::raw("f32")),
                                    ],
                                    ST::raw("i32"),
                                ),
                            ),
                            SR::field(
                                SV::Public,
                                "test_function1",
                                ST::function(
                                    SCC::Thiscall,
                                    [
                                        ("this", ST::raw("test::TestType").mut_pointer()),
                                        ("arg0", ST::raw("u32")),
                                        ("arg1", ST::raw("f32")),
                                    ],
                                    None,
                                ),
                            ),
                            make_vfunc_region(2),
                            make_vfunc_region(3),
                        ])
                        .into(),
                },
            ),
        ],
    );
}

#[test]
fn can_generate_vftable_with_indices() {
    let vftable_type = ST::raw("test::TestTypeVftable").const_pointer();
    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            TD::new([TS::vftable(
                [
                    F::new(
                        V::Public,
                        "test_function0",
                        [
                            Ar::MutSelf,
                            Ar::named("arg0", T::ident("u32")),
                            Ar::named("arg1", T::ident("f32")),
                        ],
                    )
                    .with_attributes([A::index(2)])
                    .with_return_type("i32"),
                    F::new(
                        V::Public,
                        "test_function1",
                        [
                            Ar::MutSelf,
                            Ar::named("arg0", T::ident("u32")),
                            Ar::named("arg1", T::ident("f32")),
                        ],
                    )
                    .with_attributes([A::index(5)]),
                ],
                [A::size(8)],
            )]),
        )]),
        [
            // TestType
            SID::defined_resolved(
                SV::Public,
                "test::TestType",
                SISR {
                    size: pointer_size(),
                    alignment: pointer_size(),
                    inner: STD::new()
                        .with_regions([SR::field(SV::Private, "vftable", vftable_type.clone())])
                        .with_vftable_functions(
                            vftable_type,
                            [
                                make_vfunc(0),
                                make_vfunc(1),
                                SF::new(SV::Public, "test_function0")
                                    .with_arguments([
                                        SAr::MutSelf,
                                        SAr::field("arg0", ST::raw("u32")),
                                        SAr::field("arg1", ST::raw("f32")),
                                    ])
                                    .with_return_type(ST::raw("i32")),
                                make_vfunc(3),
                                make_vfunc(4),
                                SF::new(SV::Public, "test_function1").with_arguments([
                                    SAr::MutSelf,
                                    SAr::field("arg0", ST::raw("u32")),
                                    SAr::field("arg1", ST::raw("f32")),
                                ]),
                                make_vfunc(6),
                                make_vfunc(7),
                            ],
                        )
                        .into(),
                },
            ),
            // TestTypeVftable
            SID::defined_resolved(
                SV::Public,
                "test::TestTypeVftable",
                SISR {
                    size: 8 * pointer_size(),
                    alignment: pointer_size(),
                    inner: STD::new()
                        .with_regions([
                            make_vfunc_region(0),
                            make_vfunc_region(1),
                            SR::field(
                                SV::Public,
                                "test_function0",
                                ST::function(
                                    SCC::Thiscall,
                                    [
                                        ("this", ST::raw("test::TestType").mut_pointer()),
                                        ("arg0", ST::raw("u32")),
                                        ("arg1", ST::raw("f32")),
                                    ],
                                    ST::raw("i32"),
                                ),
                            ),
                            make_vfunc_region(3),
                            make_vfunc_region(4),
                            SR::field(
                                SV::Public,
                                "test_function1",
                                ST::function(
                                    SCC::Thiscall,
                                    [
                                        ("this", ST::raw("test::TestType").mut_pointer()),
                                        ("arg0", ST::raw("u32")),
                                        ("arg1", ST::raw("f32")),
                                    ],
                                    None,
                                ),
                            ),
                            make_vfunc_region(6),
                            make_vfunc_region(7),
                        ])
                        .into(),
                },
            ),
        ],
    );
}

#[test]
fn will_propagate_calling_convention_for_impl_and_vftable() {
    let vftable_type = ST::raw("test::TestTypeVftable").const_pointer();
    assert_ast_produces_type_definitions(
        M::new()
            .with_definitions([ID::new(
                V::Public,
                "TestType",
                TD::new([TS::vftable(
                    [F::new(
                        V::Public,
                        "test_function0",
                        [
                            Ar::MutSelf,
                            Ar::named("arg0", T::ident("u32")),
                            Ar::named("arg1", T::ident("f32")),
                        ],
                    )
                    .with_return_type("i32")
                    .with_attributes([A::calling_convention("cdecl")])],
                    [],
                )]),
            )])
            .with_impls([FB::new(
                "TestType",
                [F::new(
                    V::Public,
                    "test_function",
                    [Ar::named("arg1", T::ident("i32"))],
                )
                .with_attributes([A::address(0x800_000), A::calling_convention("cdecl")])
                .with_return_type(T::ident("i32"))],
            )]),
        [
            // TestType
            SID::defined_resolved(
                SV::Public,
                "test::TestType",
                SISR {
                    size: pointer_size(),
                    alignment: pointer_size(),
                    inner: STD::new()
                        .with_regions([SR::field(SV::Private, "vftable", vftable_type.clone())])
                        .with_vftable_functions(
                            vftable_type,
                            [SF::new(SV::Public, "test_function0")
                                .with_arguments([
                                    SAr::MutSelf,
                                    SAr::field("arg0", ST::raw("u32")),
                                    SAr::field("arg1", ST::raw("f32")),
                                ])
                                .with_calling_convention(SCC::Cdecl)
                                .with_return_type(ST::raw("i32"))],
                        )
                        .with_free_functions([SF::new(SV::Public, "test_function")
                            .with_address(0x800_000)
                            .with_calling_convention(SCC::Cdecl)
                            .with_arguments([SAr::field("arg1", ST::raw("i32"))])
                            .with_return_type(ST::raw("i32"))])
                        .into(),
                },
            ),
            // TestTypeVftable
            SID::defined_resolved(
                SV::Public,
                "test::TestTypeVftable",
                SISR {
                    size: pointer_size(),
                    alignment: pointer_size(),
                    inner: STD::new()
                        .with_regions([SR::field(
                            SV::Public,
                            "test_function0",
                            ST::function(
                                SCC::Cdecl,
                                [
                                    ("this", ST::raw("test::TestType").mut_pointer()),
                                    ("arg0", ST::raw("u32")),
                                    ("arg1", ST::raw("f32")),
                                ],
                                ST::raw("i32"),
                            ),
                        )])
                        .into(),
                },
            ),
        ],
    );
}

fn make_vfunc(index: usize) -> SF {
    SF::new(SV::Private, format!("_vfunc_{}", index)).with_arguments([SAr::MutSelf])
}

fn make_vfunc_region(index: usize) -> SR {
    SR::field(
        SV::Private,
        format!("_vfunc_{}", index),
        ST::function(
            SCC::Thiscall,
            [("this", ST::raw("test::TestType").mut_pointer())],
            None,
        ),
    )
}

#[test]
fn can_define_extern_value() {
    let module1 = M::new().with_extern_values([EV::new(
        V::Public,
        "test",
        T::ident("u32").mut_pointer(),
        [A::address(0x1337)],
    )]);

    let mut semantic_state = SemanticState::new(4);
    semantic_state
        .add_module(&module1, &IP::from("module1"))
        .unwrap();
    let semantic_state = semantic_state.build().unwrap();

    let extern_value = semantic_state
        .modules()
        .get(&IP::from("module1"))
        .unwrap()
        .extern_values
        .first()
        .unwrap();

    assert_eq!(
        extern_value,
        &SEV {
            visibility: SV::Public,
            name: "test".into(),
            type_: ST::raw("u32").mut_pointer(),
            address: 0x1337
        }
    );
}

#[test]
fn can_resolve_enum() {
    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            ED::new(
                T::ident("u32"),
                [
                    ES::field_with_expr("Item0", E::IntLiteral(-2)),
                    ES::field("Item1"),
                    ES::field("Item2"),
                    ES::field_with_expr("Item3", E::IntLiteral(10)),
                    ES::field("Item4"),
                ],
                [A::singleton(0x1234)],
            ),
        )]),
        [SID::defined_resolved(
            SV::Public,
            "test::TestType",
            SISR {
                size: 4,
                alignment: 4,
                inner: SED::new(ST::raw("u32"))
                    .with_fields([
                        ("Item0", -2),
                        ("Item1", -1),
                        ("Item2", 0),
                        ("Item3", 10),
                        ("Item4", 11),
                    ])
                    .with_singleton(0x1234)
                    .into(),
            },
        )],
    );
}

#[test]
fn can_carry_backend_across() {
    let prologue = r#"
        use std::ffi::CString;
        use std::os::raw::c_char;
    "#
    .trim();

    let epilogue = r#"
        fn main() {
            println!("Hello, world!");
        }
    "#
    .trim();

    // Intentionally double-include the epilogue to test if it's correctly carried across
    let ast = M::new().with_backends([
        B::new("rust")
            .with_prologue(prologue)
            .with_epilogue(epilogue),
        B::new("rust").with_epilogue(epilogue),
    ]);
    let test_path = IP::from("test");

    let state = build_state(&ast, &test_path).unwrap();
    let module = state.modules().get(&test_path).unwrap();

    assert_eq!(
        module.backends.get("rust").unwrap(),
        &vec![
            SB {
                prologue: Some(prologue.to_string()),
                epilogue: Some(epilogue.to_string()),
            },
            SB {
                prologue: None,
                epilogue: Some(epilogue.to_string()),
            }
        ]
    );
}

#[test]
fn can_extract_copyable_and_cloneable_correctly() {
    // Check cloneable
    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            TD::new([TS::field(V::Private, "field_1", T::ident("i32"))])
                .with_attributes([A::cloneable()]),
        )]),
        [SID::defined_resolved(
            SV::Public,
            "test::TestType",
            SISR {
                size: 4,
                alignment: 4,
                inner: STD::new()
                    .with_regions([SR::field(SV::Private, "field_1", ST::raw("i32"))])
                    .with_cloneable(true)
                    .into(),
            },
        )],
    );

    // Check copyable -> copyable + cloneable
    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            TD::new([TS::field(V::Private, "field_1", T::ident("i32"))])
                .with_attributes([A::copyable()]),
        )]),
        [SID::defined_resolved(
            SV::Public,
            "test::TestType",
            SISR {
                size: 4,
                alignment: 4,
                inner: STD::new()
                    .with_regions([SR::field(SV::Private, "field_1", ST::raw("i32"))])
                    .with_copyable(true)
                    .with_cloneable(true)
                    .into(),
            },
        )],
    );
}

#[test]
fn can_extract_copyable_and_cloneable_for_enum_correctly() {
    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            ED::new(
                T::ident("u32"),
                [ES::field("Item1"), ES::field("Item2")],
                [],
            )
            .with_attributes([A::cloneable()]),
        )]),
        [SID::defined_resolved(
            SV::Public,
            "test::TestType",
            SISR {
                size: 4,
                alignment: 4,
                inner: SED::new(ST::raw("u32"))
                    .with_fields([("Item1", 0), ("Item2", 1)])
                    .with_cloneable(true)
                    .into(),
            },
        )],
    );

    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            ED::new(
                T::ident("u32"),
                [ES::field("Item1"), ES::field("Item2")],
                [],
            )
            .with_attributes([A::copyable()]),
        )]),
        [SID::defined_resolved(
            SV::Public,
            "test::TestType",
            SISR {
                size: 4,
                alignment: 4,
                inner: SED::new(ST::raw("u32"))
                    .with_fields([("Item1", 0), ("Item2", 1)])
                    .with_copyable(true)
                    .with_cloneable(true)
                    .into(),
            },
        )],
    );
}

#[test]
fn can_handle_defaultable_on_primitive_types() {
    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            TD::new([
                TS::field(V::Private, "field_1", T::ident("u64")),
                TS::field(V::Private, "field_2", T::ident("f32").array(16)),
            ])
            .with_attributes([A::defaultable(), A::align(8)]),
        )]),
        [SID::defined_resolved(
            SV::Public,
            "test::TestType",
            SISR {
                size: 72,
                alignment: 8,
                inner: STD::new()
                    .with_regions([
                        SR::field(SV::Private, "field_1", ST::raw("u64")),
                        SR::field(SV::Private, "field_2", ST::raw("f32").array(16)),
                    ])
                    .with_defaultable(true)
                    .into(),
            },
        )],
    );
}

#[test]
fn will_reject_defaultable_on_pointer() {
    assert_ast_produces_failure(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            TD::new([TS::field(
                V::Private,
                "field_1",
                T::ident("i32").mut_pointer(),
            )])
            .with_attributes([A::defaultable()]),
        )]),
        "field `field_1` of type `test::TestType` is not a defaultable type (pointer or function?)",
    );
}

#[test]
fn will_reject_defaultable_on_enum_field() {
    assert_ast_produces_failure(
        M::new().with_definitions([
            ID::new(
                V::Public,
                "TestType",
                TD::new([TS::field(V::Private, "field_1", T::ident("TestEnum"))])
                    .with_attributes([A::defaultable()]),
            ),
            ID::new(
                V::Public,
                "TestEnum",
                ED::new(T::ident("u32"), [ES::field("Item1")], []),
            ),
        ]),
        "field `field_1` of type `test::TestType` is not a defaultable type",
    );
}

#[test]
fn can_handle_defaultable_on_enum_with_default_field() {
    assert_ast_produces_failure(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            ED::new(
                T::ident("u32"),
                [ES::field("Item1"), ES::field("Item2")],
                [],
            )
            .with_attributes([A::defaultable()]),
        )]),
        "enum `test::TestType` is marked as defaultable but has no default variant set",
    );

    assert_ast_produces_failure(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            ED::new(
                T::ident("u32"),
                [
                    ES::field("Item1"),
                    ES::field("Item2").with_attributes([A::default()]),
                ],
                [],
            )
            .with_attributes([]),
        )]),
        "enum `test::TestType` has a default variant set but is not marked as defaultable",
    );

    assert_ast_produces_type_definitions(
        M::new().with_definitions([ID::new(
            V::Public,
            "TestType",
            ED::new(
                T::ident("u32"),
                [
                    ES::field("Item1"),
                    ES::field("Item2").with_attributes([A::default()]),
                ],
                [],
            )
            .with_attributes([A::defaultable()]),
        )]),
        [SID::defined_resolved(
            SV::Public,
            "test::TestType",
            SISR {
                size: 4,
                alignment: 4,
                inner: SED::new(ST::raw("u32"))
                    .with_fields([("Item1", 0), ("Item2", 1)])
                    .with_defaultable(true)
                    .with_default_index(1)
                    .into(),
            },
        )],
    );
}

#[test]
fn will_reject_defaultable_on_non_defaultable_type() {
    assert_ast_produces_failure(
        M::new().with_definitions([
            ID::new(
                V::Public,
                "TestType",
                TD::new([TS::field(
                    V::Private,
                    "field_1",
                    T::ident("TestNonDefaultable"),
                )])
                .with_attributes([A::defaultable()]),
            ),
            ID::new(V::Public, "TestNonDefaultable", TD::new([])),
        ]),
        "field `field_1` of type `test::TestType` is not a defaultable type",
    );
}
