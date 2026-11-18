subdir('icons')

# Desktop file
desktop_conf = configuration_data()
desktop_conf.set('icon', application_id)
desktop_conf.set('app-id', application_id)
desktop_file = i18n.merge_file(
  type: 'desktop',
  input: configure_file(
    input: '@0@.desktop.in.in'.format(base_id),
    output: '@BASENAME@',
    configuration: desktop_conf,
  ),
  output: '@0@.desktop'.format(application_id),
  po_dir: podir,
  install: true,
  install_dir: datadir / 'applications',
)

if desktop_file_validate.found()
  test(
    'validate-desktop',
    desktop_file_validate,
    args: [desktop_file.full_path()],
    depends: desktop_file,
  )
endif

# Metainfo file
appdata_conf = configuration_data()
appdata_conf.set('app-id', application_id)
appdata_conf.set('gettext-package', gettext_package)
appdata_file = i18n.merge_file(
  input: configure_file(
    input: '@0@.metainfo.xml.in.in'.format(base_id),
    output: '@BASENAME@',
    configuration: appdata_conf,
  ),
  output: '@0@.metainfo.xml'.format(application_id),
  po_dir: podir,
  install: true,
  install_dir: datadir / 'metainfo',
)

if appstreamcli.found()
  test(
    'validate-metainfo',
    appstreamcli,
    args: ['validate', '--no-net', '--explain', appdata_file.full_path()],
    depends: appdata_file,
  )
endif

# GSettings schema
gschema_conf = configuration_data()
gschema_conf.set('app-id', application_id)
gschema_conf.set('gettext-package', gettext_package)
gschema_conf.set('gschema-path', '/' + application_id.replace('.', '/') + '/')
gschema_file = configure_file(
  input: '@0@.gschema.xml.in'.format(base_id),
  output: '@0@.gschema.xml'.format(application_id),
  configuration: gschema_conf,
  install: true,
  install_dir: datadir / 'glib-2.0' / 'schemas',
)

# Compiled copy in the build directory, used by `meson devenv`
compiled_schemas = custom_target(
  'gschemas.compiled',
  input: gschema_file,
  output: 'gschemas.compiled',
  command: [glib_compile_schemas, '--strict', '--targetdir', '@OUTDIR@', '@OUTDIR@'],
  build_by_default: true,
)
devenv = environment()
devenv.set('GSETTINGS_SCHEMA_DIR', meson.current_build_dir())
meson.add_devenv(devenv)

test(
  'validate-gschema',
  glib_compile_schemas,
  args: ['--strict', '--dry-run', meson.current_build_dir()],
  depends: compiled_schemas,
)

# D-Bus service file, required by DBusActivatable=true
service_conf = configuration_data()
service_conf.set('app-id', application_id)
service_conf.set('bindir', bindir)
configure_file(
  input: '@0@.service.in'.format(base_id),
  output: '@0@.service'.format(application_id),
  configuration: service_conf,
  install: true,
  install_dir: datadir / 'dbus-1' / 'services',
)
