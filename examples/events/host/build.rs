use sp1_build::{build_program_with_args, BuildArgs};

fn main() {
    build_program_with_args(
        "../client",
        BuildArgs {
            binaries: vec!["decoded".to_string(), "metadata".to_string()],
            ..Default::default()
        },
    );
}
