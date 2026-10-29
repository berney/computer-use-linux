group "default" {
  targets = ["linuxserver-mod"]
}

# The mod image, loaded into the local image store.
# Publish with: docker bake --push --set linuxserver-mod.tags=<registry>/<name>:<tag>
target "linuxserver-mod" {
  context    = "."
  dockerfile = "docker/linuxserver-mod/Dockerfile"
  tags       = ["computer-use-linux-mod:dev"]
  output     = ["type=docker"]
}

# The same files as a tarball for DOCKER_MODS_SIDELOAD: mount it at /mods/computer-use-linux.tar.
# It lands next to the Dockerfile because some buildx versions will not create a missing export directory.
target "linuxserver-mod-tar" {
  inherits = ["linuxserver-mod"]
  tags     = []
  output   = ["type=tar,dest=docker/linuxserver-mod/computer-use-linux.tar"]
}
