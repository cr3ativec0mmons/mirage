cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--target-dir', meson.project_build_root() / 'src']

if get_option('profile') == 'default'
  cargo_options += ['--release']
  rust_target = 'release'
  message('Building in release mode')
else
  rust_target = 'debug'
  message('Building in debug mode')
endif

# Read by `src/config.rs` at compile time
cargo_env = [
  'MIRAGE_APP_ID=' + application_id,
  'MIRAGE_PROFILE=' + profile,
  'MIRAGE_VERSION=' + version + version_suffix,
  'MIRAGE_GETTEXT_PACKAGE=' + gettext_package,
  'MIRAGE_LOCALEDIR=' + localedir,
  'MIRAGE_PKGDATADIR=' + pkgdatadir,
]

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  # Ninja runs this through the shell, which handles `&&`. The `env` keyword
  # argument would bypass the shell, so the variables are set with env(1).
  command: [
    'env', cargo_env,
    cargo, 'build', cargo_options,
    '&&',
    'cp', 'src' / rust_target / meson.project_name(), '@OUTPUT@',
  ],
)

test(
  'cargo-test',
  find_program('env'),
  args: [cargo_env, cargo.full_path(), 'test', cargo_options],
  timeout: 600,
  is_parallel: false,
)
