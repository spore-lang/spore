use insta::assert_debug_snapshot;
use sporec_parser::parse;

fn parse_ok(src: &str) -> sporec_parser::ast::Module {
    parse(src).unwrap_or_else(|errs| {
        panic!(
            "parse failed:\n{}",
            errs.iter()
                .map(|err| err.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

#[test]
fn core_surface_ast_snapshot() {
    let module = parse_ok(
        r#"
        import std.io.File as File

        pub alias Bytes = List[U8]

        pub struct Point[T] { x: T, y: T }

        type Option[T] { Some(T), None }

        trait Display[T] {
            type Output
            fn show(self: T) -> String
        }

        effect Console {
            fn println(msg: String) -> Unit
        }

        effect IO = Console | FileRead

        handler MockConsole(output: List[Str]) handles [Console] uses [Clock] {
            impl Console {
                fn println(msg: String) -> Unit { return }
            }
        }

        @unbounded
        fn render[T](point: Point[T]) -> String where T: Display uses [Console] cost [O(n), 0, 0, 0]
        spec {
            example "origin" {
                render(Point { x: 0, y: 0 }) == "(0, 0)"
            }
            property "non_negative_identity": |x: I32 when self >= 0| x
        }
        {
            let message = f"point {point.x}";
            handle {
                perform Console.println(message);
                message
            } with {
                use MockConsole { output: [] },
                on Console.println(msg) => { msg; }
            }
        }
        "#,
    );

    assert_debug_snapshot!(module);
}
