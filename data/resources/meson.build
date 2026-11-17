resources = gnome.compile_resources(
  'resources',
  'resources.gresource.xml',
  gresource_bundle: true,
  install: true,
  install_dir: pkgdatadir,
)

# Lets `meson devenv` run the app from the build directory
devenv = environment()
devenv.set('MIRAGE_RESOURCES', resources[0].full_path())
meson.add_devenv(devenv)
