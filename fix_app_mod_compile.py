from pathlib import Path
import sys

p = Path('crates/vertexlauncher/src/app/mod.rs')
if not p.exists():
    print('error: run from repo root; missing crates/vertexlauncher/src/app/mod.rs', file=sys.stderr)
    sys.exit(1)
text = p.read_text(encoding='utf-8')
orig = text

text = text.replace(
    'desktop::open_in_file_manager(instance_root.as_path())',
    'launcher_ui::desktop::open_in_file_manager(instance_root.as_path())'
)
text = text.replace(
    'instance_root_path(&self.config.installations_root, instance)',
    'instance_root_path(std::path::Path::new(self.config.minecraft_installations_root()), instance)'
)
text = text.replace(
    'instance_root_path(&self.config.installations_root, &instance)',
    'instance_root_path(std::path::Path::new(self.config.minecraft_installations_root()), &instance)'
)

if text == orig:
    print('No changes needed or expected anchors not found.')
else:
    p.write_text(text, encoding='utf-8')
    print('Patched', p)
