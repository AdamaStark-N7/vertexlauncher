//! Every Flatpak manifest must give the launcher, and so the games it starts, the same access:
//! GPU compute and graphics, input devices, the host filesystem, audio, and a display.

use std::{fs, path::PathBuf};

const REQUIRED: &[&str] = &[
    "--device=all",
    "--filesystem=host",
    "--filesystem=host-os:ro",
    "--filesystem=/opt/rocm:ro",
    "--filesystem=/opt/cuda:ro",
    "--filesystem=/run/udev:ro",
    "--share=ipc",
    "--share=network",
    "--socket=wayland",
    "--socket=x11",
    "--socket=pulseaudio",
];

const MANIFESTS: &[&str] = &[
    "flatpak/io.github.SturdyFool10.VertexLauncher.yaml",
    "org.sturdyfool10.VertexLauncher.yml",
    "flatpak_build/io.github.SturdyFool10.VertexLauncher.yml",
    "scripts/build-flatpak.sh",
];

fn finish_args_blocks(text: &str) -> Vec<Vec<String>> {
    let mut blocks = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim_end() != "finish-args:" {
            continue;
        }
        let mut block = Vec::new();
        while let Some(next) = lines.peek() {
            let trimmed = next.trim();
            if let Some(arg) = trimmed.strip_prefix("- ") {
                block.push(arg.trim().to_owned());
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                break;
            }
            lines.next();
        }
        blocks.push(block);
    }
    blocks
}

#[test]
fn every_manifest_grants_gpu_input_filesystem_audio_and_display() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for manifest in MANIFESTS {
        let text = fs::read_to_string(root.join(manifest))
            .unwrap_or_else(|err| panic!("cannot read {manifest}: {err}"));
        let blocks = finish_args_blocks(&text);
        assert!(!blocks.is_empty(), "{manifest} has no finish-args");
        for block in blocks {
            for required in REQUIRED {
                assert!(
                    block.iter().any(|arg| arg == required),
                    "{manifest} is missing {required}"
                );
            }
        }
    }
}
